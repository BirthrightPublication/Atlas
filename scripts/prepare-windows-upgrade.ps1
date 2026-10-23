# Obtain the verified v0.2 preview or reproduce it from its pinned source.
# Used only by disposable Windows CI; no personal installation is accessed.
[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or $env:GITHUB_ACTIONS -ne 'true') { throw 'Use a disposable Windows GitHub Actions runner.' }
$repository = 'BirthrightPublication/Atlas'
$commit = '7c1b3d3a8a258e5cfceb0ae71dbb0f123d418709'
$artifactId = '12463818350'
$baseline = Join-Path $env:RUNNER_TEMP 'atlas-v020-upgrade-baseline'
if (Test-Path -LiteralPath $baseline) { throw 'Baseline directory already exists; use a clean runner.' }
New-Item -ItemType Directory -Path $baseline | Out-Null
$metadataText = & gh api "repos/$repository/actions/artifacts/$artifactId" 2>$null
$downloadAvailable = $LASTEXITCODE -eq 0
if ($downloadAvailable) {
    $metadata = $metadataText | ConvertFrom-Json
    $downloadAvailable = -not $metadata.expired
}
if ($downloadAvailable) {
    if ($metadata.name -ne 'atlas-x86_64-pc-windows-msvc' -or $metadata.workflow_run.head_sha -ne $commit) { throw 'The baseline artifact identity does not match the validated v0.2 source.' }
    $archive = Join-Path $baseline 'baseline.zip'
    # PowerShell 7.4+ preserves native stdout bytes when redirecting to a file.
    if ($PSVersionTable.PSVersion -lt [version]'7.4') { throw 'PowerShell 7.4+ is required for binary archive download.' }
    & gh api "repos/$repository/actions/artifacts/$artifactId/zip" > $archive
    if ($LASTEXITCODE -ne 0) { throw 'Baseline archive download failed.' }
    $digest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ("sha256:$digest" -cne $metadata.digest) { throw 'Baseline archive SHA-256 verification failed.' }
    Expand-Archive -LiteralPath $archive -DestinationPath $baseline
    $platform = Join-Path $baseline 'windows-x86_64'
    $manifest = Get-Content (Join-Path $platform 'artifacts.json') -Raw | ConvertFrom-Json
    $entry = @($manifest.files | Where-Object { $_.filename -ceq 'Birthright-Atlas_0.2.0_windows_x86_64-setup.exe' })
    if ($manifest.version -ne '0.2.0' -or $manifest.platform -ne 'windows-x86_64' -or $entry.Count -ne 1) { throw 'Invalid v0.2 artifact manifest.' }
    $installer = Join-Path $platform $entry[0].filename
    $hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hash -cne $entry[0].sha256 -or (Get-Item $installer).Length -ne $entry[0].size) { throw 'Baseline installer hash or length mismatch.' }
} else {
    Write-Host 'The retained v0.2 artifact is unavailable; reproducing the pinned baseline source.'
    $archive = Join-Path $baseline 'source.tar'
    & git archive --format=tar "--output=$archive" $commit
    if ($LASTEXITCODE -ne 0) { throw 'Fetch the full source history before reproducing the baseline.' }
    $source = Join-Path $baseline 'source'
    New-Item -ItemType Directory -Path $source | Out-Null
    & tar -xf $archive -C $source
    if ($LASTEXITCODE -ne 0) { throw 'Baseline source extraction failed.' }
    Push-Location $source
    try {
        & npm ci
        if ($LASTEXITCODE -ne 0) { throw 'Baseline dependency installation failed.' }
        & node scripts/build-desktop.mjs --target x86_64-pc-windows-msvc --bundles nsis
        if ($LASTEXITCODE -ne 0) { throw 'Baseline installer build failed.' }
    } finally { Pop-Location }
    $installer = Join-Path $source 'target/x86_64-pc-windows-msvc/release/bundle/nsis/Birthright Atlas_0.2.0_x64-setup.exe'
    $hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
}
"installer=$installer" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
"sha256=$hash" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
Write-Host "Verified v0.2 upgrade baseline: SHA-256 $hash"
