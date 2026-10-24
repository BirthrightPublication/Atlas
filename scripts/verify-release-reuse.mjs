// Reassemble only exact, already signed/tested artifacts of identical app code.
import { spawnSync } from "node:child_process";
import { readFile, appendFile } from "node:fs/promises";
const run = process.env.ATLAS_REUSE_RUN;
const repository = process.env.GITHUB_REPOSITORY;
if (!/^\d+$/.test(run || "") || repository !== "BirthrightPublication/Atlas")
  throw Error("Explicit same-repository signed run required.");
function command(binary, args) {
  const result = spawnSync(binary, args, { encoding: "utf8" });
  if (result.status !== 0)
    throw Error(
      "Could not verify the exact prior signed run/source. Reuse refused.",
    );
  return result.stdout;
}
const metadata = JSON.parse(
  command("gh", ["api", `repos/${repository}/actions/runs/${run}`]),
);
if (
  metadata.path !== ".github/workflows/release.yml" ||
  metadata.event !== "workflow_dispatch" ||
  metadata.head_repository.full_name !== repository ||
  !/^[a-f0-9]{40}$/.test(metadata.head_sha)
)
  throw Error(
    "Only the same production release workflow may supply artifacts.",
  );
const jobs = JSON.parse(
  command("gh", [
    "api",
    `repos/${repository}/actions/runs/${run}/jobs?per_page=100`,
  ]),
).jobs;
const signed = jobs.filter((job) => job.name.startsWith("signed-build ("));
const targets = [
  "x86_64-pc-windows-msvc",
  "x86_64-unknown-linux-gnu",
  "aarch64-apple-darwin",
  "x86_64-apple-darwin",
];
if (
  signed.length !== 4 ||
  targets.some(
    (target) =>
      signed.filter(
        (job) => job.name.includes(target) && job.conclusion === "success",
      ).length !== 1,
  )
)
  throw Error(
    "All four exact signed/tested platform jobs must have passed. Partial runs cannot be reused.",
  );
const before = JSON.parse(
  command("git", ["show", `${metadata.head_sha}:package.json`]),
);
const current = JSON.parse(await readFile("package.json", "utf8"));
if (before.version !== current.version)
  throw Error("Signed run version differs from this release checkout.");
command("git", [
  "diff",
  "--exit-code",
  metadata.head_sha,
  "HEAD",
  "--",
  "crates",
  "src",
  "src-tauri",
  "Cargo.toml",
  "Cargo.lock",
  "package.json",
  "package-lock.json",
  "index.html",
  "vite.config.ts",
  "tsconfig.json",
  "tsconfig.app.json",
  "scripts/build-desktop.mjs",
  "scripts/release-config.mjs",
]);
await appendFile(
  process.env.GITHUB_ENV,
  `ATLAS_RELEASE_SOURCE_SHA=${metadata.head_sha}\n`,
);
console.log(
  `::notice title=Verified signed artifact reuse::Run ${run}; version ${current.version}; source ${metadata.head_sha}; all four native jobs passed and application/build inputs are identical. Signatures will be checked again before draft creation.`,
);
