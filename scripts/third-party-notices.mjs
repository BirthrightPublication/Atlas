import { readFile, writeFile, mkdir, readdir, stat } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

const output = process.argv[2] || 'release/THIRD_PARTY_NOTICES.txt';
const sections = [
  'Birthright: Atlas — Third-party notices\n\nThis file retains notices from the dependencies used by this build. It does not select or grant a license for the Atlas application. Development/build tools are included for completeness. Platform system libraries retain their own licenses.\n',
];
async function notices(directory, extra = []) {
  const names = (await readdir(directory)).filter(name => /^(licen[sc]e|copying|notice)([.-]|$)/i.test(name));
  const selected = new Set([...names.map(name => path.join(directory, name)), ...extra]);
  const text = [];
  for (const filename of [...selected].sort()) {
    const info = await stat(filename).catch(() => null);
    if (info?.isFile() && info.size < 1024 * 1024) {
      text.push(`\n--- ${path.basename(filename)} ---\n${await readFile(filename, 'utf8')}`);
    }
  }
  return text.join('\n');
}
const lock = JSON.parse(await readFile('package-lock.json', 'utf8'));
for (const location of Object.keys(lock.packages).filter(name => name.startsWith('node_modules/')).sort()) {
  const manifest = await readFile(path.join(location, 'package.json'), 'utf8').catch(() => null);
  if (!manifest) continue; // Optional packages for another OS are not installed.
  const pkg = JSON.parse(manifest);
  sections.push(`\n=== npm: ${pkg.name} ${pkg.version} ===\nLicense: ${typeof pkg.license === 'string' ? pkg.license : JSON.stringify(pkg.license || pkg.licenses || 'See package notices')}\n${await notices(location)}`);
}
const metadata = spawnSync('cargo', ['metadata', '--locked', '--format-version', '1'], { encoding: 'utf8', maxBuffer: 100 * 1024 * 1024 });
if (metadata.status !== 0) throw new Error('Cannot resolve Cargo dependency notices; install locked dependencies first');
const packages = JSON.parse(metadata.stdout).packages.filter(pkg => pkg.source).sort((a, b) => a.name.localeCompare(b.name));
for (const pkg of packages) {
  const directory = path.dirname(pkg.manifest_path);
  const extras = pkg.license_file ? [path.resolve(directory, pkg.license_file)] : [];
  sections.push(`\n=== Cargo: ${pkg.name} ${pkg.version} ===\nLicense: ${pkg.license || 'See package notices'}\nSource: ${pkg.repository || pkg.source}\n${await notices(directory, extras)}`);
}
const toolchain = spawnSync('rustc', ['--print', 'sysroot'], { encoding: 'utf8' });
if (toolchain.status !== 0) throw new Error('Cannot locate Rust runtime license notices');
const rustLicenses = path.join(toolchain.stdout.trim(), 'share/doc/rust/licenses');
for (const name of (await readdir(rustLicenses)).sort()) {
  const file = path.join(rustLicenses, name);
  if ((await stat(file)).isFile()) sections.push(`\n=== Rust distribution notice: ${name} ===\n${await readFile(file, 'utf8')}`);
}
const sdkDocs = process.env.ATLAS_SDK_DOCS;
if (sdkDocs) {
  for (const name of (await readdir(sdkDocs)).sort()) {
    const copyright = path.join(sdkDocs, name, 'copyright');
    const text = await readFile(copyright, 'utf8').catch(() => null);
    if (text) sections.push(`\n=== Debian SDK/runtime notice: ${name} ===\n${text}`);
  }
}
await mkdir(path.dirname(output), { recursive: true });
await writeFile(output, sections.join('\n'));
console.log(`Retained dependency license/notice texts in ${output}`);
