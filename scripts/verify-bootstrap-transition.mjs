// Verify the exact previously delivered 0.5 preview and owner-signed bootstrap.
// No private keys, feeds, or signatures are generated/modified by this check.
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { verifyUpdate } from "./verify-update.mjs";

if (process.platform !== "win32" || process.env.GITHUB_ACTIONS !== "true")
  throw Error("Clean Windows Actions runner required.");
const filename = "Birthright-Atlas_0.5.0_windows_x86_64-setup.exe";
const baseline = await readFile(path.join("previous/windows-x86_64", filename));
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
if (
  sha(baseline) !==
  "7d94963d1223cc16fc6759aaa126420b795db073c088bc6f57a6c6296dcf9614"
)
  throw Error(
    "Unsigned 0.5.0 baseline differs from the exact delivered installer.",
  );
const key = (await readFile("release/updater-public-key.txt", "utf8")).trim();
const packet = Buffer.from(
  Buffer.from(key, "base64").toString("utf8").trim().split(/\r?\n/)[1],
  "base64",
);
if (
  sha(packet) !==
  "667267e572c171192ff432236e4db84f70e60d5348f8dbe81de8a9ec875c43ea"
)
  throw Error(
    "Bootstrap does not use the verified existing permanent identity.",
  );
const bytes = await readFile(path.join("release/windows-x86_64", filename));
const manifest = JSON.parse(
  await readFile("release/windows-x86_64/artifacts.json", "utf8"),
);
const item = manifest.files.find((f) => f.filename === filename);
if (
  !manifest.signed ||
  manifest.version !== "0.5.0" ||
  item?.sha256 !== sha(bytes) ||
  item?.size !== bytes.length
)
  throw Error("Signed bootstrap artifact identity/checksum mismatch.");
verifyUpdate(
  bytes,
  await readFile(
    path.join("release/windows-x86_64", `${filename}.sig`),
    "utf8",
  ),
  key,
  "0.5.0",
);
console.log(
  "Verified the exact unsigned 0.5.0 preview and the production-key-bearing 0.5.0 bootstrap for same-version retention testing.",
);
