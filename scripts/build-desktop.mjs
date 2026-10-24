import {
  readFile,
  writeFile,
  mkdir,
  copyFile,
  chmod,
  readdir,
  rm,
} from "node:fs/promises";
import { spawnSync } from "node:child_process";
import path from "node:path";

const args = process.argv.slice(2);
// Package public binaries/resources with normal installer permissions, even
// when the development workspace uses a private-data umask.
process.umask(0o022);
const option = (name) => {
  const i = args.indexOf(name);
  return i < 0 ? undefined : args[i + 1];
};
const rust = spawnSync("rustc", ["-vV"], { encoding: "utf8" });
if (rust.status !== 0) throw new Error("Pinned Rust toolchain is required");
const target = option("--target") || rust.stdout.match(/^host: (.+)$/m)?.[1];
const supported = {
  "x86_64-pc-windows-msvc": "nsis",
  "x86_64-unknown-linux-gnu": "appimage,deb",
  "aarch64-apple-darwin": "dmg",
  "x86_64-apple-darwin": "dmg",
};
if (!supported[target]) throw new Error("Unsupported desktop target");
let bundles = option("--bundles") || supported[target];
// A DMG-only request does not select the app updater archive. Signed macOS
// releases need the app bundle as an explicit target as well as the DMG.
if (
  args.includes("--signed") &&
  target.endsWith("-apple-darwin") &&
  !bundles.split(",").includes("app")
)
  bundles = `app,${bundles}`;
if (
  !/^(app|nsis|dmg|appimage|deb)(,(app|nsis|dmg|appimage|deb))*$/.test(bundles)
)
  throw new Error("Unsupported bundle selection");
if (bundles.split(",").includes("app") && !target.endsWith("-apple-darwin"))
  throw new Error("The app bundle target is macOS-only");
function run(binary, arguments_) {
  const result = spawnSync(binary, arguments_, {
    stdio: "inherit",
    shell: false,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
run(process.execPath, ["node_modules/typescript/bin/tsc", "-b"]);
run(process.execPath, ["node_modules/vite/bin/vite.js", "build"]);
async function publicResources(directory) {
  await chmod(directory, 0o755);
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const filename = path.join(directory, entry.name);
    if (entry.isDirectory()) await publicResources(filename);
    else if (entry.isFile()) await chmod(filename, 0o644);
  }
}
await publicResources("dist");
await publicResources("src-tauri/icons");
run("cargo", [
  "build",
  "--release",
  "--locked",
  "--target",
  target,
  "-p",
  "atlas-host",
]);
run(process.execPath, ["scripts/third-party-notices.mjs"]);
await chmod("release/THIRD_PARTY_NOTICES.txt", 0o644);
await mkdir("src-tauri/binaries", { recursive: true });
const extension = target.includes("windows") ? ".exe" : "";
await copyFile(
  path.join("target", target, "release", `atlas-host${extension}`),
  path.join("src-tauri/binaries", `atlas-host-${target}${extension}`),
);
await chmod(
  path.join("src-tauri/binaries", `atlas-host-${target}${extension}`),
  0o755,
);
await chmod(
  path.join("target", target, "release", `atlas${extension}`),
  0o755,
).catch((error) => {
  if (error.code !== "ENOENT") throw error;
});
await writeFile(
  "src-tauri/tauri.bundle.conf.json",
  JSON.stringify(
    {
      build: { beforeBuildCommand: "" },
      bundle: {
        externalBin: ["binaries/atlas-host"],
        resources: {
          "../dist/": "companion-ui/",
          "../release/THIRD_PARTY_NOTICES.txt": "THIRD_PARTY_NOTICES.txt",
        },
      },
    },
    null,
    2,
  ),
);
// These are generated bundle staging trees. Recreate them so removed assets
// and permissions from an earlier private-umask build cannot leak forward.
await rm(path.join("target", target, "release", "bundle"), {
  recursive: true,
  force: true,
});
const cli = [
  "node_modules/@tauri-apps/cli/tauri.js",
  "build",
  "--ci",
  "--target",
  target,
  "--bundles",
  bundles,
  "--config",
  "src-tauri/tauri.bundle.conf.json",
];
if (args.includes("--signed")) {
  await import("./release-config.mjs");
  cli.push("--config", "src-tauri/tauri.release.conf.json");
}
if (args.includes("--update-smoke")) {
  if (
    process.env.GITHUB_ACTIONS !== "true" ||
    target !== "x86_64-pc-windows-msvc"
  )
    throw new Error(
      "Controlled update builds require a disposable Windows CI runner",
    );
  cli.push("--features", "update-smoke");
}
cli.push("--", "--locked");
run(process.execPath, cli);
console.log(
  `Desktop and companion built for ${target}; application version ${JSON.parse(await readFile("src-tauri/tauri.conf.json", "utf8")).version}`,
);
