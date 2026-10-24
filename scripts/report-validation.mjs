// Publish only synthetic CI reports and public artifact manifests. Never profile data.
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
const allowed = new Set(['artifacts.json', 'ai-installation-benchmark.json', 'windows-installation-smoke.json', 'controlled-update-smoke.json']);
const escape = (value) => value.replaceAll('%', '%25').replaceAll('\r', '%0D').replaceAll('\n', '%0A');
async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const filename = path.join(directory, entry.name);
    if (entry.isDirectory()) await walk(filename);
    else if (allowed.has(entry.name)) {
      const report = JSON.parse(await readFile(filename, 'utf8'));
      console.log(`::notice title=${escape(entry.name)}::${escape(JSON.stringify(report))}`);
    }
  }
}
await walk('release');
