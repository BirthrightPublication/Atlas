import { readFile, mkdir, copyFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { verifyUpdate } from './verify-update.mjs';

const target = process.argv[2];
const signed = process.argv.includes('--signed');
const config = JSON.parse(await readFile('src-tauri/tauri.conf.json', 'utf8'));
const version = config.version;
const name = config.productName;
const plans = {
  'x86_64-pc-windows-msvc': { platform: 'windows-x86_64', files: [
    [`nsis/${name}_${version}_x64-setup.exe`, `Birthright-Atlas_${version}_windows_x86_64-setup.exe`, true],
  ] },
  'x86_64-unknown-linux-gnu': { platform: 'linux-x86_64', files: [
    [`appimage/${name}_${version}_amd64.AppImage`, `Birthright-Atlas_${version}_linux_x86_64.AppImage`, true],
    [`deb/${name}_${version}_amd64.deb`, `Birthright-Atlas_${version}_linux_x86_64.deb`, false],
  ] },
  'aarch64-apple-darwin': { platform: 'darwin-aarch64', files: [
    [`dmg/${name}_${version}_aarch64.dmg`, `Birthright-Atlas_${version}_macos_aarch64.dmg`, false],
    [`macos/${name}.app.tar.gz`, `Birthright-Atlas_${version}_macos_aarch64.app.tar.gz`, true],
  ] },
  'x86_64-apple-darwin': { platform: 'darwin-x86_64', files: [
    [`dmg/${name}_${version}_x64.dmg`, `Birthright-Atlas_${version}_macos_x86_64.dmg`, false],
    [`macos/${name}.app.tar.gz`, `Birthright-Atlas_${version}_macos_x86_64.app.tar.gz`, true],
  ] },
};
const plan = plans[target];
if (!plan) throw new Error('Pass an explicit supported Rust target');
if (signed && !process.env.ATLAS_UPDATE_PUBLIC_KEY) throw new Error('Release verification public key is required');
const output = path.join('release', plan.platform);
await mkdir(output, { recursive: true });
await copyFile('release/THIRD_PARTY_NOTICES.txt', path.join(output, `THIRD_PARTY_NOTICES-${plan.platform}.txt`));
const manifest = { version, platform: plan.platform, target, signed, files: [] };
for (const [sourceName, filename, updater] of plan.files) {
  // macOS updater archives are emitted only for signed updater builds.
  if (!signed && sourceName.endsWith('.app.tar.gz')) continue;
  const source = path.join('target', target, 'release', 'bundle', sourceName);
  const bytes = await readFile(source); // Missing exact artifacts fail the build.
  if (bytes.length === 0) throw new Error(`Empty artifact: ${filename}`);
  const item = { filename, sha256: createHash('sha256').update(bytes).digest('hex'), size: bytes.length, updater };
  if (signed && updater) {
    const signature = (await readFile(`${source}.sig`, 'utf8')).trim();
    verifyUpdate(bytes, signature, process.env.ATLAS_UPDATE_PUBLIC_KEY, version);
    item.signature = signature;
    await writeFile(path.join(output, `${filename}.sig`), `${signature}\n`);
  }
  await copyFile(source, path.join(output, filename));
  manifest.files.push(item);
  console.log(`::notice title=Artifact SHA-256::${filename} sha256=${item.sha256} bytes=${item.size}`);
}
await writeFile(path.join(output, 'artifacts.json'), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Staged ${manifest.files.length} exact artifacts for ${plan.platform}; signed=${signed}`);
