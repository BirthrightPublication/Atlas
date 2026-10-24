import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { verifyUpdate } from './verify-update.mjs';

const root = process.argv[2] || 'release';
const repository = process.env.GITHUB_REPOSITORY;
if (!repository || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) throw new Error('Explicit GitHub repository is required');
const publicKey = process.env.ATLAS_UPDATE_PUBLIC_KEY;
if (!publicKey) throw new Error('Verification public key is required');
const expected = ['windows-x86_64', 'linux-x86_64', 'darwin-aarch64', 'darwin-x86_64'];
const platforms = {};
const checksums = [];
let version;
for (const platform of expected) {
  const folder = path.join(root, platform);
  const manifest = JSON.parse(await readFile(path.join(folder, 'artifacts.json'), 'utf8'));
  if (!manifest.signed || manifest.platform !== platform || (version && manifest.version !== version)) {
    throw new Error(`Incomplete or inconsistent signed build: ${platform}`);
  }
  version = manifest.version;
  if (!/^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$/.test(version)) throw new Error('Invalid release version');
  let updaterCount = 0;
  for (const file of manifest.files) {
    if (path.basename(file.filename) !== file.filename) throw new Error('Artifact path must be a basename');
    const bytes = await readFile(path.join(folder, file.filename));
    const hash = createHash('sha256').update(bytes).digest('hex');
    if (hash !== file.sha256 || bytes.length !== file.size) throw new Error(`Artifact changed: ${file.filename}`);
    checksums.push(`${hash}  ${file.filename}`);
    if (file.updater) {
      updaterCount++;
      const signature = (await readFile(path.join(folder, `${file.filename}.sig`), 'utf8')).trim();
      if (signature !== file.signature) throw new Error('Signature sidecar mismatch');
      verifyUpdate(bytes, signature, publicKey, version);
      platforms[platform] = { signature, url: `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(file.filename)}` };
    }
  }
  if (updaterCount !== 1) throw new Error(`Exactly one updater artifact required for ${platform}`);
}
const notes = process.env.ATLAS_RELEASE_NOTES || `Birthright: Atlas ${version}. See the release notes and platform validation record.`;
await writeFile(path.join(root, 'latest.json'), `${JSON.stringify({ version, notes, pub_date: new Date().toISOString(), platforms }, null, 2)}\n`);
await writeFile(path.join(root, 'SHA256SUMS.txt'), `${checksums.join('\n')}\n`);
console.log(`Verified all ${expected.length} platform artifacts and signatures; draft feed ready for v${version}`);
