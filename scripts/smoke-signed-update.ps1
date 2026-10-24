# Called by the guarded installation smoke while its temporary profile junctions
# are still in place. Private signing keys and the test CA stay in RUNNER_TEMP.
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$TestRoot,
    [Parameter(Mandatory=$true)][string]$InstallDirectory,
    [Parameter(Mandatory=$true)][string]$DataDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if (-not $IsWindows -or $env:GITHUB_ACTIONS -ne 'true') { throw 'Use a disposable Windows CI runner.' }
$temporary=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')+'\'
foreach ($p in @($TestRoot,$InstallDirectory,$DataDirectory)) {
    if (-not [IO.Path]::GetFullPath($p).StartsWith($temporary,[StringComparison]::OrdinalIgnoreCase)) { throw 'Controlled updates require owned runner-temporary paths.' }
}
$repository=Split-Path -Parent $PSScriptRoot
$fixture=Join-Path $TestRoot 'signed-update-fixture'
if(Test-Path $fixture){throw 'Controlled fixture directory already exists.'}
$originals=@{}
foreach($file in @('Cargo.toml','Cargo.lock','package.json','package-lock.json','src-tauri/tauri.conf.json')) { $originals[$file]=[IO.File]::ReadAllBytes((Join-Path $repository $file)) }
$variables=@('TAURI_SIGNING_PRIVATE_KEY','TAURI_SIGNING_PRIVATE_KEY_PASSWORD','ATLAS_UPDATE_PUBLIC_KEY','ATLAS_UPDATE_ENDPOINT','ATLAS_UPDATE_TEST_CA_PEM')
$savedEnvironment=@{}
foreach($name in $variables){$savedEnvironment[$name]=[Environment]::GetEnvironmentVariable($name)}
$server=$null
$owned=[Collections.Generic.List[Diagnostics.Process]]::new()
$report=[ordered]@{status='running'; stage='Create disposable fixture'; productionSigningConfigured=$false; disposableTestCredentials=$true; transport='HTTPS with an explicit disposable CA; TLS validation enabled'; cases=@(); nativeRestart=$false}
$reportPath=Join-Path $repository 'release/windows-x86_64/controlled-update-smoke.json'
function Run-Checked([string]$Exe,[string[]]$Arguments) { & $Exe @Arguments; if($LASTEXITCODE -ne 0){throw "Controlled command failed: $Exe (exit $LASTEXITCODE)"} }
function Start-Owned([string]$Exe,[string[]]$Arguments) {
    $start=[Diagnostics.ProcessStartInfo]::new();$start.FileName=$Exe;$start.WorkingDirectory=$repository;$start.UseShellExecute=$false
    foreach($arg in $Arguments){$start.ArgumentList.Add($arg)}
    $process=[Diagnostics.Process]::Start($start);$owned.Add($process);return $process
}
try {
    Run-Checked 'node' @('scripts/update-test-feed.mjs','setup',$fixture)
    $server=Start-Owned 'node' @('scripts/update-test-feed.mjs','serve',$fixture)
    $feedPath=Join-Path $fixture 'feed.json'
    for($attempt=0;$attempt -lt 50 -and -not(Test-Path $feedPath);$attempt++){Start-Sleep -Milliseconds 100}
    if(-not(Test-Path $feedPath)){throw 'Controlled HTTPS feed did not start.'}
    $env:ATLAS_UPDATE_ENDPOINT=(Get-Content $feedPath -Raw|ConvertFrom-Json).endpoint
    $env:ATLAS_UPDATE_PUBLIC_KEY=(Get-Content (Join-Path $fixture 'update.key.pub') -Raw).Trim()
    $env:TAURI_SIGNING_PRIVATE_KEY=Join-Path $fixture 'update.key'
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD=''
    $env:ATLAS_UPDATE_TEST_CA_PEM=Get-Content (Join-Path $fixture 'ca.pem') -Raw
    $report.stage='Build disposable signed installer'
    Run-Checked 'node' @('scripts/build-desktop.mjs','--target','x86_64-pc-windows-msvc','--bundles','nsis','--signed','--update-smoke')
    $baseline=Join-Path $fixture 'baseline.exe'
    Copy-Item -LiteralPath 'target/x86_64-pc-windows-msvc/release/bundle/nsis/Birthright Atlas_0.5.0_x64-setup.exe' -Destination $baseline
    # Change only local test-build version metadata, restoring every byte below.
    foreach($file in @('package.json','package-lock.json','src-tauri/tauri.conf.json')) {
        $p=Join-Path $repository $file;$value=Get-Content $p -Raw|ConvertFrom-Json -AsHashtable;$value.version='0.5.1-update-test.1'
        if($file -eq 'package-lock.json'){$value.packages[''].version='0.5.1-update-test.1'}
        [IO.File]::WriteAllText($p,($value|ConvertTo-Json -Depth 100))
    }
    $cargo=Join-Path $repository 'Cargo.toml';[IO.File]::WriteAllText($cargo,([IO.File]::ReadAllText($cargo).Replace('version = "0.5.0"','version = "0.5.1-update-test.1"')))
    $lock=Join-Path $repository 'Cargo.lock';$content=[IO.File]::ReadAllText($lock).Replace("`r`n","`n")
    foreach($name in @('atlas-core','atlas-host','atlas-desktop')){$content=$content.Replace("name = `"$name`"`nversion = `"0.5.0`"","name = `"$name`"`nversion = `"0.5.1-update-test.1`"")}
    [IO.File]::WriteAllText($lock,$content)
    $report.stage='Build signed target 0.5.1-update-test.1'
    Run-Checked 'node' @('scripts/build-desktop.mjs','--target','x86_64-pc-windows-msvc','--bundles','nsis','--signed','--update-smoke')
    $target='target/x86_64-pc-windows-msvc/release/bundle/nsis/Birthright Atlas_0.5.1-update-test.1_x64-setup.exe'
    Copy-Item -LiteralPath $target -Destination (Join-Path $fixture 'update.exe')
    Copy-Item -LiteralPath "$target.sig" -Destination (Join-Path $fixture 'update.exe.sig')
    # Serve the exact version that produced the signed artifact, never a stale fixture constant.
    $targetVersion=[string](Get-Content (Join-Path $repository 'src-tauri/tauri.conf.json') -Raw|ConvertFrom-Json).version
    [IO.File]::WriteAllText((Join-Path $fixture 'update-version.txt'),$targetVersion)
    $report.testInstallerSha256=(Get-FileHash $target -Algorithm SHA256).Hash.ToLowerInvariant()
    $install=Start-Owned $baseline @('/S',"/D=$InstallDirectory")
    if(-not $install.WaitForExit(180000) -or $install.ExitCode -ne 0){throw 'Controlled baseline install failed.'}
    $snapshot=Join-Path $fixture 'before.json'
    foreach($case in @('older','wrong-platform','bad-signature','interrupted','malformed','missing-release','valid')) {
        $report.stage="Run controlled case $case"
        if($case -eq 'valid') {
    Run-Checked 'node' @('scripts/check-upgrade.mjs','capture',(Join-Path $InstallDirectory 'atlas-host.exe'),$DataDirectory,$snapshot)
        }
        [IO.File]::WriteAllText((Join-Path $fixture 'mode.txt'),$case)
        # Tauri forwards current arguments to NSIS /ARGS. RunAsUser may discard
        # runner environment variables when restarting as the desktop user.
        $process=Start-Owned (Join-Path $InstallDirectory 'atlas.exe') @("--atlas-update-test=$case")
        $resultPath=Join-Path $DataDirectory "controlled-update-$case.json"
        if(-not $process.WaitForExit(120000)){throw "Controlled update case $case timed out before installer handoff."}
        if($process.ExitCode -ne 0){
            $detail=if(Test-Path $resultPath){(Get-Content $resultPath -Raw|ConvertFrom-Json).error}else{'No application test report'}
            throw "Controlled update case $case failed (exit $($process.ExitCode)): $detail"
        }
        if($case -eq 'valid') {
            $marker=Join-Path $DataDirectory 'controlled-update-restarted.json'
            for($attempt=0;$attempt -lt 240 -and -not(Test-Path $resultPath);$attempt++){Start-Sleep -Milliseconds 500}
            if(-not(Test-Path $marker)){
                $installedVersion=[Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $InstallDirectory 'atlas.exe')).ProductVersion
                $detail=if(Test-Path $resultPath){(Get-Content $resultPath -Raw|ConvertFrom-Json).error}else{'No restarted test report'}
                throw "The installed signed update did not confirm restart. Installed version: $installedVersion. $detail"
            }
            $restarted=Get-Content $marker -Raw|ConvertFrom-Json
            if($restarted.version -ne '0.5.1-update-test.1' -or -not $restarted.nativeRestart){throw 'Signed update restarted the wrong version.'}
            $report.nativeRestart=$true
        }
        $result=Get-Content $resultPath -Raw|ConvertFrom-Json
        if(-not $result.passed){throw "Controlled update case $case failed: $($result.error)"}
        $report.cases+=@{name=$case;passed=$true;observedVersion=$result.version}
    }
    Run-Checked 'node' @('scripts/check-upgrade.mjs','verify',(Join-Path $InstallDirectory 'atlas-host.exe'),$DataDirectory,$snapshot)
    $report.populatedDataRetained=$true;$report.status='passed'
    Copy-Item (Join-Path $DataDirectory 'controlled-update-progress.jsonl') (Join-Path $repository 'release/windows-x86_64/controlled-update-progress.jsonl')
    Write-Host '::notice title=Controlled signed update::Passed older-version, wrong-platform, invalid-signature, interrupted-download, actual signed NSIS installation/restart and populated-data retention tests. Disposable test credentials only.'
} catch {
    $report.status='failed';$report.error=$_.Exception.Message
    $diagnostic=($_.Exception.Message -replace '%','%25' -replace "`r",'%0D' -replace "`n",'%0A')
    Write-Host "::error title=Controlled update failure::$($report.stage): $diagnostic"
    throw
} finally {
    foreach($file in $originals.Keys){[IO.File]::WriteAllBytes((Join-Path $repository $file),$originals[$file])}
    foreach($name in $variables){[Environment]::SetEnvironmentVariable($name,$savedEnvironment[$name])}
    foreach($process in $owned){$process.Refresh();if(-not $process.HasExited){$process.Kill($true);$process.WaitForExit(10000)|Out-Null}}
    # Remove only this fixture's disposable private keys, never owner credentials.
    foreach($name in @('update.key','ca.key','server.key')){ $key=Join-Path $fixture $name;if(Test-Path $key){Remove-Item -LiteralPath $key -Force} }
    $report|ConvertTo-Json -Depth 10|Set-Content $reportPath -Encoding utf8NoBOM
}
