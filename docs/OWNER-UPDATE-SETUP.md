# Activate Atlas updates once

Current status: owner setup and publication are pending. A disposable-key update
smoke test is not production activation. The existing v0.4 preview cannot learn a
new verification key through its updater. Install the production-configured v0.5
bootstrap manually once; later releases must use the same signing identity.

## 1. Generate and retain the identity on your Windows computer

Use a trusted Windows computer with Node 24 and Git installed. In PowerShell:

```powershell
git clone https://github.com/BirthrightPublication/Atlas.git Atlas-release
Set-Location Atlas-release
git switch codex/atlas-v0.5-production-updater
npm ci
$atlasKeyDirectory = Join-Path $env:LOCALAPPDATA 'AtlasReleaseKeys'
New-Item -ItemType Directory -Force $atlasKeyDirectory | Out-Null
$atlasKeyFile = Join-Path $atlasKeyDirectory 'atlas-updater.key'
if (Test-Path $atlasKeyFile) { throw 'An identity already exists. Reuse it; do not overwrite it.' }
npx tauri signer generate --write-keys $atlasKeyFile
```

Choose a strong password at the private prompt. Keep the `.key` file, its `.pub`
file and password in an owner-controlled encrypted backup. Do not generate the
permanent key in this cloud workspace or disposable CI. Do not send the private
key or password in chat, source, PRs, issues or logs. The public `.pub` value is
safe to distribute. Reuse this identity across subsequent releases.

## 2. Configure GitHub

Open repository **Settings → Environments → New environment**, name it
`release-signing`, and apply the review/access protection available to your
repository. Configure these environment entries:

| Type | Name | Value |
| --- | --- | --- |
| Secret | `TAURI_SIGNING_PRIVATE_KEY` | Full contents of the private `.key` file |
| Secret | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password |
| Variable | `ATLAS_UPDATE_PUBLIC_KEY` | Full contents of the `.pub` file |

Enter values directly in GitHub's protected UI. Do not put the private file path
in the secret: the remote runner needs the actual encrypted key contents.
Updater signatures do not establish Windows Authenticode publisher signing.

## 3. Integrate the reviewed application and workflow

Review the v0.5 integration PR to `main`. It preserves all stacked v0.1–v0.4
commits. Merge only after owner approval and the required candidate checks.
Until `.github/workflows/release.yml` exists on the default branch, GitHub's
normal manual workflow launcher is unavailable. Merely pushing a development
branch does not activate it. No branch is merged by this setup guide.

After integration, open **Actions → Prepare signed draft release → Run workflow**.
Select the reviewed release branch/commit and enter the release notes. The
workflow checks environment configuration before building, verifies signatures,
and creates a draft only after every platform artifact is present. It does not
publish a stable release automatically.

## 4. Bootstrap v0.5.0

Review the signed draft's source, `latest.json`, `SHA256SUMS.txt`, installer,
signature sidecars and platform map. Download the Windows x64 installer from the
draft and compare its SHA-256 with `SHA256SUMS.txt` using:

```powershell
Get-FileHash .\Birthright-Atlas_0.5.0_windows_x86_64-setup.exe -Algorithm SHA256
```

Export a backup from Atlas, close it and install into the existing location.
Keep the existing application identity/data directory; do not uninstall/delete
personal data. Confirm the installed version is 0.5.0 and Updates reports a
configured trusted feed. The same permanent public key is compiled into this
bootstrap. Installation itself does not publish an update.

## 5. Prove a real published upgrade

Populate an isolated Windows test profile with stay, transportation, budget,
document, reminder, chat, notification and preferences. Prepare v0.5.1 with the
same production key through the real workflow. Review its complete draft and
approve publication. Publish the stable release only after explicit owner
approval; all referenced assets must already be uploaded and verified.

The stable endpoint is:
<https://github.com/BirthrightPublication/Atlas/releases/latest/download/latest.json>.
Verify it returns the expected version and exact Windows x64 URL/signature.
Open the actual installed 0.5.0 app, open Updates and click **Update Now**. Observe
backup, real transfer progress, signature verification, installation and restart.
Confirm 0.5.1, SQLite integrity, documents, chats, preferences, notification and
companion state survive. Record the actual result in `VALIDATION-v0.5.md`.
This acceptance remains pending until performed against that published release.
Do not publish a test-only version simply to imply success.

## Recovery

A failed transfer/signature check leaves the installed application and backup
available. Retry a reachable verified release; never disable TLS or signatures.
If installation fails, close task-owned Atlas/companion processes and rerun the
verified installer into the existing location. Preserve the entire user-data
folder first. Restore a backup only with a schema-compatible Atlas version; do
not open a newer database with an older executable. Models and mailbox secrets
are outside the replaceable application directory. Keep the permanent signing
identity backed up; losing it requires a planned manual trust transition.
