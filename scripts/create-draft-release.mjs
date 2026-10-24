import { readFile, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import path from "node:path";

const feed = JSON.parse(await readFile("release/latest.json", "utf8"));
const platforms = [
  "windows-x86_64",
  "linux-x86_64",
  "darwin-aarch64",
  "darwin-x86_64",
];
const files = [
  "release/latest.json",
  "release/SHA256SUMS.txt",
  "release/updater-public-key.txt",
  "release/signing-identity.json",
  "release/windows-x86_64/windows-installation-smoke.json",
];
for (const platform of platforms) {
  const folder = path.join("release", platform);
  const manifest = JSON.parse(
    await readFile(path.join(folder, "artifacts.json"), "utf8"),
  );
  if (!manifest.signed || manifest.version !== feed.version)
    throw new Error("Inconsistent release artifacts");
  files.push(path.join(folder, `THIRD_PARTY_NOTICES-${platform}.txt`));
  for (const file of manifest.files) {
    files.push(path.join(folder, file.filename));
    if (file.updater) files.push(path.join(folder, `${file.filename}.sig`));
  }
}
const notes = `${process.env.ATLAS_RELEASE_NOTES || ""}\n\nThis is a draft. Complete installation, upgrade, mailbox and physical-device checks before publishing. Updater signatures authenticate artifacts; OS publisher signing/notarization is separate.\n`;
await writeFile("release/notes.md", notes);
const source = process.env.ATLAS_RELEASE_SOURCE_SHA || process.env.GITHUB_SHA;
if (!/^[a-f0-9]{40}$/.test(source || ""))
  throw Error("Exact validated release source SHA is required");
const result = spawnSync(
  "gh",
  [
    "release",
    "create",
    `v${feed.version}`,
    "--draft",
    "--target",
    source,
    "--title",
    `Birthright: Atlas ${feed.version}`,
    "--notes-file",
    "release/notes.md",
    ...files,
  ],
  { encoding: "utf8", shell: false },
);
if (result.error) throw result.error;
if (result.stdout) console.log(result.stdout.trim());
if (result.status !== 0) {
  let diagnostic = result.stderr || "GitHub CLI returned no diagnostic";
  if (process.env.GH_TOKEN)
    diagnostic = diagnostic.replaceAll(process.env.GH_TOKEN, "[redacted]");
  diagnostic = diagnostic
    .replaceAll("%", "%25")
    .replaceAll("\r", "%0D")
    .replaceAll("\n", "%0A");
  console.error(
    `::error title=Signed draft release creation failed::${diagnostic}`,
  );
}
process.exit(result.status ?? 1);
