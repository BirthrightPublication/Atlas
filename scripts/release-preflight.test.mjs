import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtemp,
  mkdir,
  writeFile,
  readFile,
  readdir,
  rm,
  symlink,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

test("preflight diagnoses configuration, verifies the existing identity and never emits private material", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "atlas-preflight-test-"));
  const cli = path.resolve("node_modules/@tauri-apps/cli/tauri.js");
  const script = path.resolve("scripts/release-preflight.mjs");
  const cleanEnv = {
    ...process.env,
    ATLAS_UPDATE_PUBLIC_KEY: "",
    TAURI_SIGNING_PRIVATE_KEY: "",
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "",
    TAURI_SIGNING_PRIVATE_KEY_PATH: "",
    GITHUB_STEP_SUMMARY: "",
  };
  const run = (values = {}) =>
    spawnSync(process.execPath, [script], {
      cwd: directory,
      env: { ...cleanEnv, ...values },
      encoding: "utf8",
      timeout: 40000,
    });
  const generate = async (name) => {
    const filename = path.join(directory, name);
    const result = spawnSync(
      process.execPath,
      [
        cli,
        "signer",
        "generate",
        "--ci",
        "--password",
        "synthetic-test-password",
        "--write-keys",
        filename,
      ],
      { encoding: "utf8" },
    );
    assert.equal(
      result.status,
      0,
      "Synthetic test key generation failed; output withheld",
    );
    return {
      public: await readFile(`${filename}.pub`, "utf8"),
      private: await readFile(filename, "utf8"),
    };
  };
  try {
    await mkdir(path.join(directory, "src-tauri"));
    await writeFile(
      path.join(directory, "src-tauri/tauri.conf.json"),
      '{"version":"0.5.0"}',
    );
    await writeFile(
      path.join(directory, "package.json"),
      '{"version":"0.5.0"}',
    );
    await symlink(
      path.resolve("node_modules"),
      path.join(directory, "node_modules"),
      process.platform === "win32" ? "junction" : "dir",
    );
    const missing = run();
    assert.equal(missing.status, 1);
    for (const name of [
      "ATLAS_UPDATE_PUBLIC_KEY",
      "TAURI_SIGNING_PRIVATE_KEY",
      "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
    ])
      assert.match(missing.stderr, new RegExp(name));
    const malformed = run({
      ATLAS_UPDATE_PUBLIC_KEY: "untrusted-input-do-not-echo",
    });
    assert.equal(malformed.status, 1);
    assert.match(malformed.stderr, /not a Tauri public key/);
    assert.ok(!malformed.stderr.includes("untrusted-input-do-not-echo"));
    const key = await generate("synthetic.key");
    const values = {
      ATLAS_UPDATE_PUBLIC_KEY: key.public,
      TAURI_SIGNING_PRIVATE_KEY: key.private,
      TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "synthetic-test-password",
    };
    const filePath = run({
      ...values,
      TAURI_SIGNING_PRIVATE_KEY: "C:\\private-example\\synthetic.key",
    });
    assert.equal(filePath.status, 1);
    assert.match(filePath.stderr, /contains a file path/);
    assert.ok(!filePath.stderr.includes("C:\\private-example"));
    const malformedPrivate = run({
      ...values,
      TAURI_SIGNING_PRIVATE_KEY: "malformed-synthetic-secret-do-not-echo",
    });
    assert.equal(malformedPrivate.status, 1);
    assert.match(
      malformedPrivate.stderr,
      /not valid Tauri private-key file contents/,
    );
    assert.ok(
      !malformedPrivate.stderr.includes(
        "malformed-synthetic-secret-do-not-echo",
      ),
    );
    const wrongPassword = run({
      ...values,
      TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "wrong-test-password",
    });
    assert.equal(wrongPassword.status, 1);
    assert.match(wrongPassword.stderr, /PASSWORD does not unlock/);
    const another = await generate("another-synthetic.key");
    const mismatch = run({
      ...values,
      ATLAS_UPDATE_PUBLIC_KEY: another.public,
    });
    assert.equal(mismatch.status, 1);
    assert.match(mismatch.stderr, /does not verify/);
    const valid = run(values);
    assert.equal(
      valid.status,
      0,
      "Valid synthetic preflight failed; output withheld",
    );
    const identity = JSON.parse(
      await readFile(
        path.join(directory, "release/signing-identity.json"),
        "utf8",
      ),
    );
    assert.equal(identity.signedProbeVerified, true);
    assert.match(identity.publicKeySha256, /^[a-f0-9]{64}$/);
    assert.deepEqual((await readdir(path.join(directory, "release"))).sort(), [
      "signing-identity.json",
      "updater-public-key.txt",
    ]);
    const overlayResult = spawnSync(
      process.execPath,
      [path.resolve("scripts/release-config.mjs")],
      {
        cwd: directory,
        env: {
          ...cleanEnv,
          ...values,
          ATLAS_UPDATE_PUBLIC_KEY: `${key.public.trim()}\r\n`,
          ATLAS_UPDATE_ENDPOINT:
            "https://github.com/BirthrightPublication/Atlas/releases/latest/download/latest.json",
        },
        encoding: "utf8",
      },
    );
    assert.equal(overlayResult.status, 0);
    const overlayText = await readFile(
      path.join(directory, "src-tauri/tauri.release.conf.json"),
      "utf8",
    );
    const overlay = JSON.parse(overlayText);
    assert.equal(overlay.plugins.updater.pubkey, key.public.trim());
    assert.equal(overlay.plugins.updater.requireSignedVersion, true);
    assert.equal(overlay.bundle.createUpdaterArtifacts, true);
    assert.ok(
      !overlayText.includes(key.private.trim()),
      "Private key entered release config",
    );
    for (const result of [
      filePath,
      malformedPrivate,
      wrongPassword,
      mismatch,
      valid,
    ]) {
      assert.ok(
        !`${result.stdout}${result.stderr}`.includes(key.private.trim()),
        "Private key leaked",
      );
      assert.ok(
        !`${result.stdout}${result.stderr}`.includes("synthetic-test-password"),
        "Password leaked",
      );
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
