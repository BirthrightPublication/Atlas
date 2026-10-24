// Read/download only. Uses the owner's PUBLIC key, never a private signing key.
import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { verifyUpdate } from "./verify-update.mjs";

if (process.platform !== "win32" || process.env.GITHUB_ACTIONS !== "true")
  throw Error("Use a clean disposable Windows Actions runner.");
const repo = "BirthrightPublication/Atlas";
const tag = process.env.ATLAS_BOOTSTRAP_TAG;
const target = process.env.ATLAS_EXPECTED_RELEASE_VERSION;
if (
  !/^v\d+\.\d+\.\d+$/.test(tag || "") ||
  !/^\d+\.\d+\.\d+$/.test(target || "")
)
  throw Error("Explicit stable bootstrap tag and target version required.");
const base = tag.slice(1);
const before = base.split(".").map(Number),
  after = target.split(".").map(Number);
const differing = before.findIndex((n, i) => n !== after[i]);
if (
  differing < 0 ||
  ![...before, ...after].every(Number.isSafeInteger) ||
  before[differing] >= after[differing]
)
  throw Error("The published target must be newer than the bootstrap.");
const key = process.env.ATLAS_UPDATE_PUBLIC_KEY?.trim();
if (!key)
  throw Error(
    "Owner public verification key required; no private key is used.",
  );
function run(binary, args, capture = false) {
  const result = spawnSync(binary, args, {
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
    timeout: 600000,
  });
  if (result.status !== 0)
    throw Error(
      "Production release verification/download failed. No update acceptance is claimed.",
    );
  return result.stdout;
}
run(process.execPath, ["scripts/verify-published-release.mjs"]);
const published = JSON.parse(
  run("gh", ["api", `repos/${repo}/releases/latest`], true),
);
if (
  published.draft ||
  published.prerelease ||
  published.tag_name !== `v${target}`
)
  throw Error(
    "The expected newer release is not the real published stable release.",
  );
const folder = path.resolve("release/windows-x86_64");
await mkdir(folder, { recursive: true });
const filename = `Birthright-Atlas_${base}_windows_x86_64-setup.exe`;
const names = [
  filename,
  `${filename}.sig`,
  "SHA256SUMS.txt",
  "updater-public-key.txt",
  "signing-identity.json",
];
run("gh", [
  "release",
  "download",
  tag,
  "--repo",
  repo,
  "--dir",
  folder,
  ...names.flatMap((n) => ["--pattern", n]),
]);
const bytes = await readFile(path.join(folder, filename));
const sha256 = createHash("sha256").update(bytes).digest("hex");
const sums = (
  await readFile(path.join(folder, "SHA256SUMS.txt"), "utf8")
).split(/\r?\n/);
if (!sums.includes(`${sha256}  ${filename}`))
  throw Error("Bootstrap SHA-256 does not match its release checksums.");
const publicKey = (
  await readFile(path.join(folder, "updater-public-key.txt"), "utf8")
).trim();
if (publicKey !== key)
  throw Error(
    "Bootstrap identity differs from the configured permanent public key. Never rotate keys to pass this test.",
  );
const signature = (
  await readFile(path.join(folder, `${filename}.sig`), "utf8")
).trim();
verifyUpdate(bytes, signature, key, base);
await writeFile(
  path.join(folder, "artifacts.json"),
  JSON.stringify(
    {
      version: base,
      platform: "windows-x86_64",
      signed: true,
      files: [
        { filename, size: bytes.length, sha256, updater: true, signature },
      ],
    },
    null,
    2,
  ),
);
await writeFile(
  path.join(folder, "production-preparation.json"),
  JSON.stringify(
    {
      bootstrap: base,
      target,
      bootstrapSha256: sha256,
      publishedFeedVerified: true,
      bootstrapSignatureVerified: true,
      samePublicIdentity: true,
      privateKeyUsed: false,
    },
    null,
    2,
  ),
);
console.log(
  `Verified production ${base} bootstrap and published ${target} update with the same permanent identity. Native UI acceptance is next.`,
);
