import { writeFile } from "node:fs/promises";

const key = process.env.ATLAS_UPDATE_PUBLIC_KEY?.trim();
const feed = process.env.ATLAS_UPDATE_ENDPOINT;
if (
  !key ||
  !process.env.TAURI_SIGNING_PRIVATE_KEY ||
  !feed ||
  new URL(feed).protocol !== "https:"
) {
  throw new Error(
    "Signed release requires the signing key, public verification key, and HTTPS feed",
  );
}
// The private signing key stays in the process environment, never this overlay.
// Share the normalized public identity with the Tauri compiler child process.
process.env.ATLAS_UPDATE_PUBLIC_KEY = key;
await writeFile(
  "src-tauri/tauri.release.conf.json",
  JSON.stringify(
    {
      bundle: { createUpdaterArtifacts: true },
      plugins: {
        updater: { pubkey: key, endpoints: [feed], requireSignedVersion: true },
      },
    },
    null,
    2,
  ),
);
console.log("Prepared signed release overlay");
