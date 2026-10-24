// Read-only verification of the real stable feed. This never installs or publishes.
import { verifyUpdate } from "./verify-update.mjs";
import { createHash } from "node:crypto";
const repo = "BirthrightPublication/Atlas";
const key = process.env.ATLAS_UPDATE_PUBLIC_KEY;
if (!key)
  throw Error(
    "Set the public verification key locally; no private key is needed.",
  );
const get = async (url) => {
  for (let hop = 0; hop < 9; hop++) {
    const parsed = new URL(url);
    if (parsed.protocol !== "https:" || parsed.username || parsed.password)
      throw Error("HTTPS URLs without embedded credentials required");
    const response = await fetch(parsed, {
      redirect: "manual",
      signal: AbortSignal.timeout(120000),
    });
    if ([301, 302, 303, 307, 308].includes(response.status)) {
      const location = response.headers.get("location");
      if (!location) throw Error("Missing redirect target");
      url = new URL(location, parsed).href;
      await response.body?.cancel();
      continue;
    }
    if (!response.ok)
      throw Error(`Published endpoint returned HTTP ${response.status}`);
    return response;
  }
  throw Error("Too many published-artifact redirects");
};
const release = await (
  await get(`https://api.github.com/repos/${repo}/releases/latest`)
).json();
if (release.draft || release.prerelease)
  throw Error("A published stable release is required");
const manifest = await (
  await get(`https://github.com/${repo}/releases/latest/download/latest.json`)
).json();
if (
  process.env.ATLAS_EXPECTED_RELEASE_VERSION &&
  manifest.version !== process.env.ATLAS_EXPECTED_RELEASE_VERSION
)
  throw Error(
    "The published manifest is not the explicitly expected release version",
  );
if (
  release.tag_name !== `v${manifest.version}` ||
  !/^\d+\.\d+\.\d+$/.test(manifest.version)
)
  throw Error("Stable feed/tag mismatch");
const sums = await (
  await get(
    `https://github.com/${repo}/releases/download/${release.tag_name}/SHA256SUMS.txt`,
  )
).text();
const expected = {
  "windows-x86_64": `Birthright-Atlas_${manifest.version}_windows_x86_64-setup.exe`,
  "linux-x86_64": `Birthright-Atlas_${manifest.version}_linux_x86_64.AppImage`,
  "darwin-aarch64": `Birthright-Atlas_${manifest.version}_macos_aarch64.app.tar.gz`,
  "darwin-x86_64": `Birthright-Atlas_${manifest.version}_macos_x86_64.app.tar.gz`,
};
for (const [platform, name] of Object.entries(expected)) {
  const item = manifest.platforms?.[platform];
  const url = `https://github.com/${repo}/releases/download/${release.tag_name}/${encodeURIComponent(name)}`;
  if (item?.url !== url || !item.signature)
    throw Error(`Invalid exact platform map: ${platform}`);
  const bytes = Buffer.from(await (await get(url)).arrayBuffer());
  const sha = createHash("sha256").update(bytes).digest("hex");
  if (!sums.split(/\r?\n/).includes(`${sha}  ${name}`))
    throw Error(`Published checksum mismatch: ${platform}`);
  verifyUpdate(bytes, item.signature, key, manifest.version);
  console.log(`${platform}: SHA-256 and version-bound signature verified`);
}
console.log(
  `Published Atlas ${manifest.version} verified. Native Update Now acceptance remains separate.`,
);
