import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { verifyUpdate } from './verify-update.mjs';

test('real Tauri signatures authenticate bytes, key, signed version and trusted comment', async () => {
  const directory = await mkdtemp(path.join(tmpdir(), 'atlas-signature-'));
  const cli = path.resolve('node_modules/@tauri-apps/cli/tauri.js');
  const run = (...args) => {
    const result = spawnSync(process.execPath, [cli, 'signer', ...args], { encoding: 'utf8' });
    // CLI output can contain ephemeral test signing material. Never print it.
    assert.equal(result.status, 0, `Tauri signing command exited ${result.status}`);
  };
  try {
    const keyPath = path.join(directory, 'key');
    const artifactPath = path.join(directory, 'artifact');
    await writeFile(artifactPath, 'Synthetic artifact for update verification tests only.');
    run('generate', '--ci', '--password', '', '--write-keys', keyPath);
    run('sign', '--private-key-path', keyPath, '--password', '', '--app-version', '0.1.0', artifactPath);
    const bytes = await readFile(artifactPath);
    const signature = await readFile(`${artifactPath}.sig`, 'utf8');
    const key = await readFile(`${keyPath}.pub`, 'utf8');
    assert.equal(verifyUpdate(bytes, signature, key, '0.1.0'), true);
    assert.throws(() => verifyUpdate(Buffer.concat([bytes, Buffer.from('tampered')]), signature, key, '0.1.0'), /Artifact signature/);
    assert.throws(() => verifyUpdate(bytes, signature, key, '9.9.9'), /expected release version/);
    const changedComment = Buffer.from(Buffer.from(signature.trim(), 'base64').toString('utf8').replace('version:0.1.0', 'version:9.9.9')).toString('base64');
    assert.throws(() => verifyUpdate(bytes, changedComment, key, '9.9.9'), /Trusted comment signature/);
    run('generate', '--ci', '--password', '', '--write-keys', path.join(directory, 'another-key'));
    const anotherKey = await readFile(path.join(directory, 'another-key.pub'), 'utf8');
    assert.throws(() => verifyUpdate(bytes, signature, anotherKey, '0.1.0'), /key ID mismatch/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
