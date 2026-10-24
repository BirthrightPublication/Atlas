import { readFile, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

const feed = JSON.parse(await readFile('release/latest.json', 'utf8'));
const platforms = ['windows-x86_64', 'linux-x86_64', 'darwin-aarch64', 'darwin-x86_64'];
const files = ['release/latest.json', 'release/SHA256SUMS.txt', 'release/windows-x86_64/windows-installation-smoke.json'];
for (const platform of platforms) {
  const folder = path.join('release', platform);
  const manifest = JSON.parse(await readFile(path.join(folder, 'artifacts.json'), 'utf8'));
  if (!manifest.signed || manifest.version !== feed.version) throw new Error('Inconsistent release artifacts');
  files.push(path.join(folder, `THIRD_PARTY_NOTICES-${platform}.txt`));
  for (const file of manifest.files) {
    files.push(path.join(folder, file.filename));
    if (file.updater) files.push(path.join(folder, `${file.filename}.sig`));
  }
}
const notes = `${process.env.ATLAS_RELEASE_NOTES || ''}\n\nThis is a draft. Complete installation, upgrade, mailbox and physical-device checks before publishing. Updater signatures authenticate artifacts; OS publisher signing/notarization is separate.\n`;
await writeFile('release/notes.md', notes);
const result = spawnSync('gh', ['release', 'create', `v${feed.version}`, '--draft', '--target', process.env.GITHUB_SHA, '--title', `Birthright: Atlas ${feed.version}`, '--notes-file', 'release/notes.md', ...files], { stdio: 'inherit', shell: false });
if (result.error) throw result.error;
process.exit(result.status ?? 1);
