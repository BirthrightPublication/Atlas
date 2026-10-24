import { readFile } from "node:fs/promises";
const publicKey = process.env.ATLAS_UPDATE_PUBLIC_KEY?.trim();
const lines = Buffer.from(publicKey || "", "base64")
  .toString("utf8")
  .trim()
  .split(/\r?\n/);
if (lines.length !== 2 || Buffer.from(lines[1] || "", "base64").length !== 42)
  throw new Error(
    "Configure release-signing environment variable ATLAS_UPDATE_PUBLIC_KEY with the Tauri .pub contents. See docs/OWNER-UPDATE-SETUP.md.",
  );
if (!process.env.TAURI_SIGNING_PRIVATE_KEY)
  throw new Error(
    "Configure owner-controlled TAURI_SIGNING_PRIVATE_KEY in release-signing. Never generate the permanent identity in CI.",
  );
if (!process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD)
  throw new Error(
    "Configure TAURI_SIGNING_PRIVATE_KEY_PASSWORD in release-signing.",
  );
const config = JSON.parse(await readFile("src-tauri/tauri.conf.json", "utf8"));
const pkg = JSON.parse(await readFile("package.json", "utf8"));
if (config.version !== pkg.version || !/^\d+\.\d+\.\d+$/.test(config.version))
  throw new Error(
    "Stable release needs consistent, plain major.minor.patch version metadata.",
  );
console.log(
  `Release ${config.version} has required signing configuration; signatures will be verified against this public identity after building.`,
);
