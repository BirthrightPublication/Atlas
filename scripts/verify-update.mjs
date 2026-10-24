// Minisign envelope verification using Node's audited Ed25519/BLAKE2 primitives.
// No signing keys are read here. Tauri signatures are base64-wrapped minisign text.
import { createHash, createPublicKey, verify } from 'node:crypto';

function envelope(value) {
  return Buffer.from(value.trim(), 'base64').toString('utf8').trim().split(/\r?\n/);
}

export function verifyUpdate(bytes, signature, publicKey, expectedVersion) {
  const publicLines = envelope(publicKey);
  const signatureLines = envelope(signature);
  if (publicLines.length !== 2 || signatureLines.length !== 4 || !signatureLines[2].startsWith('trusted comment: ')) {
    throw new Error('Malformed minisign envelope');
  }
  const key = Buffer.from(publicLines[1], 'base64');
  const sig = Buffer.from(signatureLines[1], 'base64');
  const global = Buffer.from(signatureLines[3], 'base64');
  if (key.length !== 42 || sig.length !== 74 || global.length !== 64 || key.subarray(0, 2).toString() !== 'Ed') {
    throw new Error('Unsupported minisign key or signature');
  }
  if (!key.subarray(2, 10).equals(sig.subarray(2, 10))) throw new Error('Signing key ID mismatch');
  const algorithm = sig.subarray(0, 2).toString();
  if (!['Ed', 'ED'].includes(algorithm)) throw new Error('Unsupported minisign algorithm');
  const publicObject = createPublicKey({
    key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), key.subarray(10)]),
    format: 'der', type: 'spki',
  });
  const signedBytes = algorithm === 'ED' ? createHash('blake2b512').update(bytes).digest() : bytes;
  if (!verify(null, signedBytes, publicObject, sig.subarray(10))) throw new Error('Artifact signature is invalid');
  const comment = signatureLines[2].slice('trusted comment: '.length);
  if (!verify(null, Buffer.concat([sig.subarray(10), Buffer.from(comment)]), publicObject, global)) {
    throw new Error('Trusted comment signature is invalid');
  }
  // New Tauri CLI binds the app version in the signed comment. A release cannot
  // pair an older legitimate artifact with an inflated manifest version.
  if (expectedVersion && !comment.split(/\s+/).includes(`version:${expectedVersion}`)) {
    throw new Error('Signature does not bind the expected release version');
  }
  return true;
}
