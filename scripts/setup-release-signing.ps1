#Requires -Version 7.0
# Run only on the owner's trusted Windows computer. Reuses the permanent key;
# never generates, replaces, prints, or uploads an unverified signing identity.
[CmdletBinding()]
param(
    [string] $KeyPath = (Join-Path $env:LOCALAPPDATA 'AtlasReleaseKeys/atlas-updater.key'),
    [ValidatePattern('^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$')]
    [string] $Repository = 'BirthrightPublication/Atlas',
    [switch] $Upload
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or $env:GITHUB_ACTIONS -eq 'true' -or $env:CI -eq 'true') {
    throw 'Use your trusted Windows computer, not cloud or CI. This helper reuses an existing permanent identity.'
}
foreach ($file in @($KeyPath, "$KeyPath.pub")) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
        throw 'The existing private key and matching .pub file must both exist. Locate your retained AtlasReleaseKeys files or follow docs/OWNER-UPDATE-SETUP.md; this helper will never overwrite or generate a replacement.'
    }
}
$node = (Get-Command node -ErrorAction Stop).Source
if ($Upload) { $github = (Get-Command gh -ErrorAction Stop).Source }
$repositoryRoot = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $repositoryRoot 'node_modules/@tauri-apps/cli/tauri.js'))) {
    throw 'Run npm ci in this Atlas checkout first.'
}

function Invoke-ProtectedTool([string] $Binary, [string[]] $Arguments, [AllowNull()][string] $InputValue) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Binary
    $start.WorkingDirectory = $repositoryRoot
    $start.UseShellExecute = $false
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::Start($start)
    try {
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if ($null -ne $InputValue) { $process.StandardInput.Write($InputValue) }
        $process.StandardInput.Close()
        if (-not $process.WaitForExit(60000)) {
            $process.Kill($true)
            throw 'The protected command timed out. No command output or credentials were printed.'
        }
        # Do not echo captured output: third-party programs may include supplied
        # values in errors. GitHub authentication and environment setup are
        # checked before secrets are loaded.
        $null = $stdout.GetAwaiter().GetResult()
        $null = $stderr.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) { throw 'Protected command failed; output withheld. Verify gh auth login, repository access, release-signing environment, the matching .pub file and the existing key password. No key was replaced.' }
    } finally { $process.Dispose() }
}

if ($Upload) {
    Invoke-ProtectedTool $github @('auth', 'status') $null
    Invoke-ProtectedTool $github @('api', "repos/$Repository/environments/release-signing") $null
}
$names = @('TAURI_SIGNING_PRIVATE_KEY', 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'ATLAS_UPDATE_PUBLIC_KEY', 'GITHUB_STEP_SUMMARY')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$password = Read-Host 'Password for your EXISTING Atlas signing key' -AsSecureString
$pointer = [IntPtr]::Zero
try {
    $pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($password)
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    $env:TAURI_SIGNING_PRIVATE_KEY = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $KeyPath)).Trim()
    $env:ATLAS_UPDATE_PUBLIC_KEY = [IO.File]::ReadAllText((Resolve-Path -LiteralPath "$KeyPath.pub")).Trim()
    [Environment]::SetEnvironmentVariable('GITHUB_STEP_SUMMARY', $null, 'Process')
    Invoke-ProtectedTool $node @((Join-Path $PSScriptRoot 'release-preflight.mjs')) $null
    $identity = Get-Content -LiteralPath (Join-Path $repositoryRoot 'release/signing-identity.json') -Raw | ConvertFrom-Json
    Write-Host "Existing signing identity verified. Public-key SHA-256: $($identity.publicKeySha256)"
    if ($Upload) {
        Invoke-ProtectedTool $github @('secret', 'set', 'TAURI_SIGNING_PRIVATE_KEY', '--env', 'release-signing', '--repo', $Repository) $env:TAURI_SIGNING_PRIVATE_KEY
        Invoke-ProtectedTool $github @('secret', 'set', 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD', '--env', 'release-signing', '--repo', $Repository) $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
        Invoke-ProtectedTool $github @('variable', 'set', 'ATLAS_UPDATE_PUBLIC_KEY', '--env', 'release-signing', '--repo', $Repository) $env:ATLAS_UPDATE_PUBLIC_KEY
        Write-Host 'Uploaded the verified existing key/password as secrets and its .pub contents as the public variable. No release was published.'
    } else {
        Write-Host 'Validation only. Add -Upload to configure GitHub with this verified existing identity.'
    }
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process') }
    if ($pointer -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer) }
    $password.Dispose()
}
