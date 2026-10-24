# Activate Atlas updates once

The owner has merged PR #5 into `main` and created `release-signing`. The signed
workflow is available in Actions. The owner corrected the public-key format and
password. The existing permanent key/password pair has signed a version-bound
proof which the public key verified. Signed release builds are now running;
publication and real in-app update acceptance remain separate approval/test
steps. See
[v0.6 activation evidence](VALIDATION-v0.6.md).

The installed unsigned v0.5 preview cannot learn a new verification key through
its updater. Install the production-configured v0.5 bootstrap manually once;
later releases must use the same signing identity. Preserve the data directory;
do not uninstall or delete it.

**If you already generated the permanent key, skip step 1.** Keep that key and
its password. Reuse the configured identity; step 2 includes recovery guidance
if a future configuration check fails.

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

| Type     | Name                                 | Value                                    |
| -------- | ------------------------------------ | ---------------------------------------- |
| Secret   | `TAURI_SIGNING_PRIVATE_KEY`          | Full contents of the private `.key` file |
| Secret   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password                             |
| Variable | `ATLAS_UPDATE_PUBLIC_KEY`            | Full contents of the `.pub` file         |

Enter values directly in GitHub's protected UI. Do not put the private file path
in the secret: the remote runner needs the actual encrypted key contents.
Updater signatures do not establish Windows Authenticode publisher signing.

### Correct the signing-key password

Open **Settings → Environments → release-signing → Environment secrets →
TAURI_SIGNING_PRIVATE_KEY_PASSWORD → Update**. Enter the original password you
chose at the Tauri key-generation prompt, then save. This is the signing-key
password, not your GitHub login password. Keep its exact case and characters.
Do not add quotation marks, leading/trailing spaces or a newline.

GitHub does not reveal existing secret values. This cloud workspace cannot
retrieve or recover that password. Keep it out of chat; enter it directly in
GitHub. The optional Windows helper below checks the existing local key and
password before uploading, which also checks against incorrectly copied key
contents. Do not generate or overwrite a permanent identity to bypass this
check. If the password is lost, locate the original retained password/backup
before proceeding. No signed installer can be produced until the key unlocks.

### Public-key setting (format corrected)

On your trusted Windows computer, the guide's default public-key file is
`%LOCALAPPDATA%\AtlasReleaseKeys\atlas-updater.key.pub`. In PowerShell:

```powershell
Get-Content -LiteralPath (Join-Path $env:LOCALAPPDATA 'AtlasReleaseKeys\atlas-updater.key.pub') -Raw | Set-Clipboard
```

Open **Settings → Environments → release-signing → Environment variables →
ATLAS_UPDATE_PUBLIC_KEY**, paste the copied contents and save. The value is the
whole public file's encoded text. File paths, quotation marks, private-key
contents and placeholder text are invalid. Do not generate a replacement key.
Rerun the failed signed workflow after saving. Its preflight now verifies a real
version-bound proof using the existing private key and password and reports the
public-key SHA-256 before starting native builds.

### Optional Windows helper for the existing identity

Use PowerShell 7 on the trusted Windows computer, with Node 24, GitHub CLI and
this reviewed checkout. Run `npm ci` and `gh auth login` first, then:

```powershell
# Checks the existing local key/password pair; does not change GitHub.
./scripts/setup-release-signing.ps1
# After local verification, sends the existing secrets directly to GitHub and
# the matching .pub contents to the environment variable.
./scripts/setup-release-signing.ps1 -Upload
```

Use `-KeyPath 'C:\your\retained\atlas-updater.key'` if the original key is
elsewhere. The helper never creates or overwrites a key, hides captured command
output, sends secrets through standard input rather than command arguments,
restores its process environment and refuses CI execution. It does not publish
a release. Retain the encrypted backup of your original identity.

## 3. Integrate the reviewed application and workflow

PR #5 has been merged by the owner and preserves all stacked v0.1–v0.4 commits.
`.github/workflows/release.yml` now exists on `main`. Further changes still
require review and owner-authorized integration; this helper never merges.

After integration, open **Actions → Prepare signed draft release → Run workflow**.
Select the reviewed release branch/commit and enter the release notes. The
workflow checks environment configuration before building, verifies signatures,
installs the actual signed Windows bootstrap over a populated v0.4 test profile,
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
document, reminder, chat, notification and preferences. Prepare v0.6.0 with the
same production key through the real workflow. Review its complete draft and
approve publication. Publish the stable release only after explicit owner
approval; all referenced assets must already be uploaded and verified.

The stable endpoint is:
<https://github.com/BirthrightPublication/Atlas/releases/latest/download/latest.json>.
Verify it returns the expected version and exact Windows x64 URL/signature.
Open the actual installed 0.5.0 app, open Updates and click **Update Now**. Observe
backup, real transfer progress, signature verification, installation and restart.
Confirm 0.6.0, SQLite integrity, documents, chats, preferences, notification and
companion state survive. Record the actual result in `VALIDATION-v0.6.md`.
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
