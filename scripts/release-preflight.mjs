import { readFile, writeFile, mkdir, mkdtemp, rm, appendFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import { verifyUpdate } from "./verify-update.mjs";

// Annotations remain readable when runner logs cannot be downloaded. Never
// print secret values, private paths, or signing-process output.
const failures = [];
const publicKey = process.env.ATLAS_UPDATE_PUBLIC_KEY?.trim();
const lines = Buffer.from(publicKey || "", "base64").toString("utf8").trim().split(/\r?\n/);
const packet = Buffer.from(lines[1] || "", "base64");
if (!publicKey) failures.push("Missing environment VARIABLE ATLAS_UPDATE_PUBLIC_KEY. Add the full atlas-updater.key.pub contents to Settings > Environments > release-signing > Environment variables (not secrets).");
else if (lines.length !== 2 || !lines[0].startsWith("untrusted comment:") || packet.length !== 42 || packet.subarray(0, 2).toString() !== "Ed") failures.push("ATLAS_UPDATE_PUBLIC_KEY is not a Tauri public key. Paste the full .pub file contents, without quotes or a file path, into the release-signing environment VARIABLE.");
if (!process.env.TAURI_SIGNING_PRIVATE_KEY?.trim()) failures.push("Missing environment SECRET TAURI_SIGNING_PRIVATE_KEY. Add the full existing private .key file contents directly in GitHub release-signing; never paste it into chat or generate a replacement identity in CI.");
if (!process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD) failures.push("Missing environment SECRET TAURI_SIGNING_PRIVATE_KEY_PASSWORD. Add the password chosen when the existing permanent key was generated. Do not replace or overwrite the key.");

async function report() {
  for (const message of failures) console.error(`::error title=Release signing configuration::${message}`);
  if (process.env.GITHUB_STEP_SUMMARY) await appendFile(process.env.GITHUB_STEP_SUMMARY, `## Signing preflight failed\n\n${failures.map(message => `- ${message}`).join("\n")}\n\nNo installer or release was created. See docs/OWNER-UPDATE-SETUP.md.\n`);
  process.exitCode = 1;
}

if (failures.length) {
  await report();
} else {
  const config = JSON.parse(await readFile("src-tauri/tauri.conf.json", "utf8"));
  const pkg = JSON.parse(await readFile("package.json", "utf8"));
  if (config.version !== pkg.version || !/^\d+\.\d+\.\d+$/.test(config.version)) {
    failures.push("Stable release needs matching plain major.minor.patch versions in package.json and src-tauri/tauri.conf.json.");
  } else {
    const directory = await mkdtemp(path.join(tmpdir(), "atlas-signing-preflight-"));
    try {
      const probe = path.join(directory, "proof.txt");
      await writeFile(probe, `Atlas ${config.version} signing identity verification. Not an application or update artifact.\n`);
      const signingEnvironment = { ...process.env };
      delete signingEnvironment.TAURI_SIGNING_PRIVATE_KEY_PATH;
      const result = spawnSync(process.execPath, [path.resolve("node_modules/@tauri-apps/cli/tauri.js"), "signer", "sign", "--app-version", config.version, probe], { env: signingEnvironment, encoding: "utf8", timeout: 30000, stdio: ["ignore", "pipe", "pipe"] });
      if (result.status !== 0) failures.push("The existing private key could not sign with its configured password. Check that TAURI_SIGNING_PRIVATE_KEY contains the entire encrypted .key file (not a path) and its password is correct. Keep this identity; do not rotate it. Signing output was withheld to protect credentials.");
      else {
        try {
          verifyUpdate(await readFile(probe), await readFile(`${probe}.sig`, "utf8"), publicKey, config.version);
        } catch {
          failures.push("The configured public key does not verify the private key's version-bound signature. Use the .pub file belonging to the existing permanent private key; do not generate a new identity.");
        }
      }
      if (!failures.length) {
        const fingerprint = createHash("sha256").update(packet).digest("hex");
        await mkdir("release", { recursive: true });
        await writeFile("release/updater-public-key.txt", `${publicKey}\n`);
        await writeFile("release/signing-identity.json", `${JSON.stringify({ publicKeySha256: fingerprint, signedProbeVerified: true, version: config.version }, null, 2)}\n`);
        console.log(`::notice title=Permanent signing identity verified::Atlas ${config.version}; public-key SHA-256 ${fingerprint}. Private key/password pair and version-bound signature verified. No release published.`);
        if (process.env.GITHUB_STEP_SUMMARY) await appendFile(process.env.GITHUB_STEP_SUMMARY, `## Signing identity verified\n\nVersion: ${config.version}\n\nPublic-key SHA-256: \`${fingerprint}\`\n\nThe existing permanent key signed a synthetic proof and its public key verified it. No release has been published.\n`);
      }
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  }
  if (failures.length) await report();
}
