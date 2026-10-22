# Requires a Windows GitHub Actions runner with Node 24 from actions/setup-node.
# This tests the staged unsigned preview installer; it does not bypass signing,
# SmartScreen, WebView2, or Windows security checks. No developer server is used.
# Windows execution is required: Linux syntax review is not installation evidence.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string] $InstallerPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $IsWindows -or $env:GITHUB_ACTIONS -ne 'true') {
    throw 'This smoke test only runs on a disposable Windows GitHub Actions runner. It never uses an existing personal Atlas profile.'
}
if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP) -or -not [IO.Directory]::Exists($env:RUNNER_TEMP)) {
    throw 'RUNNER_TEMP must name an existing temporary directory on the Windows runner.'
}

$repository = Split-Path -Parent $PSScriptRoot
$config = Get-Content -LiteralPath (Join-Path $repository 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
$productName = [string] $config.productName
$identifier = [string] $config.identifier
$version = [string] $config.version
$windowTitle = [string] $config.app.windows[0].title
if ($productName -ne 'Birthright Atlas' -or $identifier -ne 'org.birthright.atlas') {
    throw 'The product identity changed. Review the installed binary, data-directory, and shortcut expectations before running this smoke test.'
}
if ($config.bundle.windows.nsis.installMode -ne 'currentUser') {
    throw 'This smoke test expects the configured current-user NSIS installer; machine-wide installation is not supported by this test.'
}

$installer = (Get-Item -LiteralPath $InstallerPath).FullName
$stagingDirectory = [IO.Path]::GetFullPath((Join-Path $repository 'release/windows-x86_64'))
$expectedFilename = "Birthright-Atlas_${version}_windows_x86_64-setup.exe"
if ([IO.Path]::GetDirectoryName($installer) -ine $stagingDirectory -or [IO.Path]::GetFileName($installer) -cne $expectedFilename) {
    throw "Pass the exact staged NSIS artifact: release/windows-x86_64/$expectedFilename. Wildcards and unstaged binaries are not accepted."
}
$manifest = Get-Content -LiteralPath (Join-Path $stagingDirectory 'artifacts.json') -Raw | ConvertFrom-Json
$entries = @($manifest.files | Where-Object { $_.filename -ceq $expectedFilename })
if ($manifest.platform -ne 'windows-x86_64' -or $manifest.version -ne $version -or $entries.Count -ne 1) {
    throw 'The staged artifact manifest does not identify exactly one installer for this version and platform.'
}
$installerHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
if ($installerHash -cne ([string] $entries[0].sha256).ToLowerInvariant() -or (Get-Item -LiteralPath $installer).Length -ne $entries[0].size) {
    throw 'The exact staged installer failed its SHA-256 or length check. Rebuild and stage it; verification is never disabled.'
}

$runId = [Guid]::NewGuid().ToString('N')
$testRoot = Join-Path ([IO.Path]::GetFullPath($env:RUNNER_TEMP)) "atlas-install-smoke-$runId"
$installDirectory = Join-Path $testRoot 'installed'
$isolatedRoaming = Join-Path $testRoot 'AppData/Roaming'
$isolatedLocal = Join-Path $testRoot 'AppData/Local'
$databaseDirectory = Join-Path $isolatedRoaming $identifier
$databasePath = Join-Path $databaseDirectory 'atlas.sqlite3'
$installedExecutable = Join-Path $installDirectory 'atlas.exe'
$ownedJunctions = [Collections.Generic.List[object]]::new()
$ownedProcesses = [Collections.Generic.List[Diagnostics.Process]]::new()
$stage = 'Prepare isolated profile'
$cleanupFailures = [Collections.Generic.List[string]]::new()
$reportPath = Join-Path $stagingDirectory 'windows-installation-smoke.json'
$report = [ordered]@{
    status = 'running'
    version = $version
    platform = 'windows-x86_64'
    installer = $expectedFilename
    installerSha256 = $installerHash
    installedBinary = 'atlas.exe'
    isolatedTemporaryProfile = $true
    developerServerUsed = $false
    userInteractive = [Environment]::UserInteractive
}

function Get-OptionalProperty($Object, [string] $Name) {
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) { return $null }
    return $property.Value
}

function Add-OwnedDataJunction([string] $KnownFolder, [string] $TargetRoot) {
    if ([string]::IsNullOrWhiteSpace($KnownFolder) -or -not [IO.Directory]::Exists($KnownFolder)) {
        throw 'Windows did not provide an existing application-data known folder. Isolated native validation cannot proceed.'
    }
    $link = Join-Path $KnownFolder $identifier
    if ($null -ne (Get-Item -LiteralPath $link -Force -ErrorAction SilentlyContinue)) {
        throw "Refusing to touch an existing Atlas data path: $link. Use a clean ephemeral runner."
    }
    $target = Join-Path $TargetRoot $identifier
    New-Item -ItemType Directory -Path $target -Force | Out-Null
    try {
        New-Item -ItemType Junction -Path $link -Value $target -ErrorAction Stop | Out-Null
    } catch {
        throw "Cannot create the isolated Atlas data junction at '$link': $($_.Exception.Message). No real profile fallback is allowed."
    }
    $ownedJunctions.Add([pscustomobject]@{ Path = $link; Target = $target })
    $junction = Get-Item -LiteralPath $link -Force
    $junctionTargets = @($junction.Target)
    if (($junction.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0 -or $junction.LinkType -ne 'Junction' -or $junctionTargets.Count -ne 1 -or [IO.Path]::GetFullPath([string] $junctionTargets[0]) -ine [IO.Path]::GetFullPath($target)) {
        throw "The Atlas data path did not resolve to the owned temporary junction: $link."
    }
}

function Start-InstalledAtlas([string] $LaunchLabel) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $installedExecutable
    $start.WorkingDirectory = $installDirectory
    $start.UseShellExecute = $false
    $start.Environment['APPDATA'] = $isolatedRoaming
    $start.Environment['LOCALAPPDATA'] = $isolatedLocal
    # Tauri also uses Windows known-folder APIs, which ignore these environment
    # overrides. The guarded per-application junctions isolate those paths too.
    $process = [Diagnostics.Process]::Start($start)
    if ($null -eq $process) { throw "$LaunchLabel did not create an Atlas process." }
    $ownedProcesses.Add($process)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($clock.Elapsed.TotalSeconds -lt 20) {
        $process.Refresh()
        if ($process.HasExited) {
            throw "$LaunchLabel exited before readiness (exit code $($process.ExitCode)). Verify the installed WebView2 runtime and Windows native prerequisites; no developer-server fallback was attempted."
        }
        if ($process.MainWindowHandle -ne [IntPtr]::Zero -and $process.MainWindowTitle -eq $windowTitle -and [IO.File]::Exists($databasePath) -and (Get-Item -LiteralPath $databasePath).Length -ge 100) {
            return [pscustomobject]@{ Process = $process; ReadyMilliseconds = [int] $clock.Elapsed.TotalMilliseconds; Title = $process.MainWindowTitle }
        }
        Start-Sleep -Milliseconds 250
    }
    throw "$LaunchLabel did not expose the '$windowTitle' native window and a nonempty SQLite database within 20 seconds. Process alive=$(-not $process.HasExited); window handle=$($process.MainWindowHandle); observed title='$($process.MainWindowTitle)'; database present=$([IO.File]::Exists($databasePath)). Check the runner's desktop session, WebView2, and application-data isolation."
}

function Close-InstalledAtlas([Diagnostics.Process] $Process, [string] $LaunchLabel) {
    $Process.Refresh()
    if ($Process.HasExited) { throw "$LaunchLabel exited unexpectedly before the normal close check (exit code $($Process.ExitCode))." }
    if (-not $Process.CloseMainWindow()) { throw "$LaunchLabel did not accept a normal native-window close request." }
    if (-not $Process.WaitForExit(10000)) { throw "$LaunchLabel did not exit within 10 seconds of closing its window." }
    if ($Process.ExitCode -ne 0) { throw "$LaunchLabel closed with exit code $($Process.ExitCode)." }
}

try {
    if (Test-Path -LiteralPath $testRoot) { throw 'The unique smoke-test directory unexpectedly already exists.' }
    New-Item -ItemType Directory -Path $testRoot | Out-Null
    $realRoaming = [Environment]::GetFolderPath([Environment+SpecialFolder]::ApplicationData)
    $realLocal = [Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)
    # Reject both existing paths before creating either link. No personal data
    # is moved, renamed, deleted, or read during this test.
    foreach ($root in @($realRoaming, $realLocal)) {
        if ([string]::IsNullOrWhiteSpace($root) -or $null -ne (Get-Item -LiteralPath (Join-Path $root $identifier) -Force -ErrorAction SilentlyContinue)) {
            throw 'An Atlas application-data path already exists or a Windows known folder is unavailable. Use a clean disposable runner.'
        }
    }
    Add-OwnedDataJunction $realRoaming $isolatedRoaming
    Add-OwnedDataJunction $realLocal $isolatedLocal

    $programs = [Environment]::GetFolderPath([Environment+SpecialFolder]::Programs)
    $startMenuFolder = Get-OptionalProperty $config.bundle.windows.nsis 'startMenuFolder'
    $shortcutDirectory = if ([string]::IsNullOrWhiteSpace($startMenuFolder)) { $programs } else { Join-Path $programs $startMenuFolder }
    $shortcutPath = Join-Path $shortcutDirectory "$productName.lnk"
    $desktopShortcut = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::DesktopDirectory)) "$productName.lnk"
    if ((Test-Path -LiteralPath $shortcutPath) -or (Test-Path -LiteralPath $desktopShortcut) -or (Test-Path -LiteralPath "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$productName")) {
        throw 'An Atlas shortcut or current-user installation registration already exists. Refusing to overwrite an earlier installation during a smoke test.'
    }

    $stage = 'Silent install exact staged NSIS artifact'
    # NSIS /D consumes the remaining command line: it must be the last argument
    # and the directory itself must not be quoted, including when it has spaces.
    $install = Start-Process -FilePath $installer -ArgumentList "/S /D=$installDirectory" -PassThru
    $ownedProcesses.Add($install)
    if (-not $install.WaitForExit(180000)) { throw 'The NSIS silent installation exceeded three minutes. Check WebView2 bootstrapper access and the installer logs; installation has not passed.' }
    if ($install.ExitCode -ne 0) { throw "The NSIS installer returned exit code $($install.ExitCode). A reboot request or failed prerequisite is not a completed installation." }
    if (-not [IO.File]::Exists($installedExecutable)) { throw "NSIS completed without the expected installed atlas.exe at '$installDirectory'." }
    foreach ($resource in @('atlas-host.exe', 'companion-ui/index.html', 'THIRD_PARTY_NOTICES.txt')) {
        if (-not [IO.File]::Exists((Join-Path $installDirectory $resource)) -or (Get-Item -LiteralPath (Join-Path $installDirectory $resource)).Length -eq 0) {
            throw "The installed application is missing the nonempty packaged resource '$resource'."
        }
    }
    $report.packagedResources = @('atlas-host.exe', 'companion-ui/index.html', 'THIRD_PARTY_NOTICES.txt')
    $report.installerExitCode = $install.ExitCode
    if (-not [IO.File]::Exists($shortcutPath)) { throw "NSIS did not create the configured normal Start Menu launcher '$productName.lnk'." }

    $stage = 'Verify installed launcher'
    $shell = New-Object -ComObject WScript.Shell
    try {
        $shortcut = $shell.CreateShortcut($shortcutPath)
        if ([IO.Path]::GetFullPath($shortcut.TargetPath) -ine [IO.Path]::GetFullPath($installedExecutable) -or -not [string]::IsNullOrWhiteSpace($shortcut.Arguments)) {
            throw 'The Start Menu launcher does not directly target the installed atlas.exe with normal launch arguments.'
        }
    } finally {
        if ($null -ne $shell) { [Runtime.InteropServices.Marshal]::ReleaseComObject($shell) | Out-Null }
    }
    $report.startMenuShortcut = "$productName.lnk"

    $stage = 'First installed launch'
    if (-not [Environment]::UserInteractive) {
        throw 'NSIS installed the app and its launcher, but this runner has no interactive desktop session. Native-window startup and restart remain unverified; use an interactive Windows runner.'
    }
    $first = Start-InstalledAtlas 'First installed launch'
    $report.firstLaunch = @{ readyMilliseconds = $first.ReadyMilliseconds; nativeWindowTitle = $first.Title }
    Close-InstalledAtlas $first.Process 'First installed launch'
    $databaseCreatedAt = (Get-Item -LiteralPath $databasePath).CreationTimeUtc

    # A marker in this isolated database proves restart retention, beyond merely
    # seeing a file path twice. Node 24 is already pinned by this workflow.
    $probePath = Join-Path $testRoot 'check-persistence.cjs'
    @'
const { DatabaseSync } = require('node:sqlite');
const [filename, mode, marker] = process.argv.slice(2);
const db = new DatabaseSync(filename, { readOnly: mode === 'verify' });
try {
  if (db.prepare('PRAGMA quick_check').get().quick_check !== 'ok') throw new Error('Atlas SQLite integrity check failed');
  if (db.prepare('PRAGMA user_version').get().user_version < 1) throw new Error('Installed Atlas did not initialize its versioned schema');
  if (mode === 'seed') {
    db.prepare('INSERT INTO settings(key, value) VALUES (?, ?)').run('windows_installer_smoke', JSON.stringify({ synthetic: true, marker }));
  } else {
    const row = db.prepare('SELECT value FROM settings WHERE key = ?').get('windows_installer_smoke');
    if (!row || JSON.parse(row.value).marker !== marker) throw new Error('The installed app did not retain its SQLite data across restart');
  }
  console.log(`Installed Atlas SQLite ${mode === 'seed' ? 'initialized' : 'retained the synthetic persistence marker'}; integrity: ok`);
} finally { db.close(); }
'@ | Set-Content -LiteralPath $probePath -Encoding utf8NoBOM
    & node $probePath $databasePath 'seed' $runId
    if ($LASTEXITCODE -ne 0) { throw "First-launch SQLite validation failed (node exit $LASTEXITCODE)." }

    $stage = 'Restart installed application'
    $second = Start-InstalledAtlas 'Restarted installed launch'
    $report.secondLaunch = @{ readyMilliseconds = $second.ReadyMilliseconds; nativeWindowTitle = $second.Title }
    Close-InstalledAtlas $second.Process 'Restarted installed launch'
    if (-not [IO.File]::Exists($databasePath) -or (Get-Item -LiteralPath $databasePath).CreationTimeUtc -ne $databaseCreatedAt) {
        throw 'The application database was missing or replaced during restart.'
    }
    & node $probePath $databasePath 'verify' $runId
    if ($LASTEXITCODE -ne 0) { throw "Restart SQLite persistence validation failed (node exit $LASTEXITCODE)." }
    $report.sqliteIntegrity = 'ok'
    $report.persistenceMarkerRetained = $true
    $report.status = 'passed'
} catch {
    $report.status = 'failed'
    $report.failedStage = $stage
    $report.failure = $_.Exception.Message
    throw "Windows installed-app smoke test failed at '$stage': $($_.Exception.Message)"
} finally {
    $processCleanupComplete = $true
    foreach ($process in $ownedProcesses) {
        try {
            $process.Refresh()
            if (-not $process.HasExited) {
                $process.Kill($true)
                if (-not $process.WaitForExit(10000)) { throw "Owned process $($process.Id) did not stop." }
            }
        } catch { $processCleanupComplete = $false; $cleanupFailures.Add($_.Exception.Message) }
    }
    # Delete only junction entries created by this test. Never recursively remove
    # known folders or follow links into the test target (or any real data).
    if (-not $processCleanupComplete) {
        $cleanupFailures.Add('Owned process cleanup was incomplete; temporary data junctions were deliberately retained rather than removed under a running app.')
    }
    foreach ($owned in $(if ($processCleanupComplete) { $ownedJunctions } else { @() })) {
        try {
            $junction = Get-Item -LiteralPath $owned.Path -Force
            $junctionTargets = @($junction.Target)
            if (($junction.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0 -or $junction.LinkType -ne 'Junction' -or $junctionTargets.Count -ne 1 -or [IO.Path]::GetFullPath([string] $junctionTargets[0]) -ine [IO.Path]::GetFullPath($owned.Target)) {
                throw "Owned junction changed unexpectedly; refusing to remove '$($owned.Path)'."
            }
            [IO.Directory]::Delete($owned.Path)
        } catch { $cleanupFailures.Add($_.Exception.Message) }
    }
    if ($cleanupFailures.Count -gt 0) {
        $report.status = 'failed'
        $report.cleanupFailures = @($cleanupFailures)
    }
    $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $reportPath -Encoding utf8NoBOM
    if ($cleanupFailures.Count -gt 0) { throw "Windows smoke cleanup failed: $($cleanupFailures -join '; ')" }
}

Write-Host "Windows installation smoke passed: exact NSIS install, installed atlas.exe, Start Menu launcher, two native launches, and SQLite persistence. Evidence: $reportPath"
Write-Host "Temporary installation and synthetic data remain under $testRoot for runner diagnostics; no user profile was deleted."
