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
function invoke(args) {
  return spawnSync("gh", args, { encoding: "utf8", shell: false });
}
function completed(result) {
  if (result.error) throw result.error;
  if (result.status === 0) return result.stdout;
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
  process.exit(result.status ?? 1);
}
const repository = process.env.GITHUB_REPOSITORY;
if (repository !== "BirthrightPublication/Atlas")
  throw Error("Explicit Atlas repository required");
const tag = `v${feed.version}`;
const lookup = invoke(["api", `repos/${repository}/releases/tags/${tag}`]);
if (lookup.status === 0) {
  const draft = JSON.parse(lookup.stdout);
  if (
    !draft.draft ||
    draft.prerelease ||
    draft.tag_name !== tag ||
    draft.target_commitish !== source ||
    draft.assets.length !== 0
  )
    throw Error(
      "Refusing to modify a published, populated, or different-source release. Only an empty exact-source draft can be adopted.",
    );
  // Repository authorization can create the empty draft when Actions cannot.
  // Never clobber assets or change the draft/publication state.
  for (const filename of files) await readFile(filename);
  completed(invoke(["release", "upload", tag, ...files]));
  await writeFile("release/draft-notes.json", JSON.stringify({ body: notes }));
  completed(
    invoke([
      "api",
      "--method",
      "PATCH",
      `repos/${repository}/releases/${draft.id}`,
      "--input",
      "release/draft-notes.json",
    ]),
  );
  console.log(
    `Attached all verified assets to the existing exact-source draft ${tag}. No release was published.`,
  );
  process.exit(0);
}
if (!lookup.stderr?.includes("404")) completed(lookup);
const result = invoke([
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
]);
console.log(completed(result)?.trim());
