import test from "node:test";
import assert from "node:assert/strict";
import https from "node:https";
import { spawn, execFile } from "node:child_process";
import { once } from "node:events";
import { promisify } from "node:util";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const execute = promisify(execFile);
test(
  "controlled HTTPS feed uses the signed build version and preserves rejection cases",
  { timeout: 30000 },
  async (t) => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "atlas-feed-test-"));
    let server;
    t.after(async () => {
      if (server && server.exitCode === null) {
        const stopped = once(server, "exit");
        server.kill();
        await stopped;
      }
      await rm(directory, { recursive: true, force: true });
    });
    await execute(process.execPath, [
      "scripts/update-test-feed.mjs",
      "setup",
      directory,
    ]);
    server = spawn(
      process.execPath,
      ["scripts/update-test-feed.mjs", "serve", directory],
      { stdio: "ignore" },
    );
    let feed;
    for (let attempt = 0; attempt < 100; attempt++) {
      try {
        feed = JSON.parse(
          await readFile(path.join(directory, "feed.json"), "utf8"),
        );
        break;
      } catch {
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
    }
    assert.ok(feed, "The real controlled HTTPS server must start");
    const ca = await readFile(path.join(directory, "ca.pem"));
    const get = (url, trusted = true) =>
      new Promise((resolve, reject) => {
        const request = https.get(
          url,
          { ...(trusted ? { ca } : {}), timeout: 3000 },
          (response) => {
            const chunks = [];
            response.on("data", (chunk) => chunks.push(chunk));
            response.on("error", reject);
            response.on("aborted", () =>
              reject(new Error("Interrupted response")),
            );
            response.on("end", () =>
              resolve({
                status: response.statusCode,
                bytes: Buffer.concat(chunks),
              }),
            );
          },
        );
        request.on("error", reject);
        request.on("timeout", () =>
          request.destroy(new Error("Request timed out")),
        );
      });
    await assert.rejects(
      get(feed.endpoint.replace("/latest.json", "/health"), false),
    );
    const artifact = Buffer.alloc(16384, 42);
    await writeFile(path.join(directory, "update.exe"), artifact);
    await writeFile(
      path.join(directory, "update.exe.sig"),
      "synthetic-signature",
    );
    await writeFile(
      path.join(directory, "update-version.txt"),
      "0.4.1-update-test.1",
    );
    for (const scenario of [
      "older",
      "wrong-platform",
      "bad-signature",
      "interrupted",
      "valid",
    ]) {
      await writeFile(path.join(directory, "mode.txt"), scenario);
      const response = await get(feed.endpoint);
      assert.equal(response.status, 200);
      const manifest = JSON.parse(response.bytes);
      assert.equal(
        manifest.version,
        scenario === "older" ? "0.0.0" : "0.4.1-update-test.1",
      );
      const platform =
        scenario === "wrong-platform" ? "darwin-aarch64" : "windows-x86_64";
      assert.deepEqual(Object.keys(manifest.platforms), [platform]);
      const url = manifest.platforms[platform].url;
      if (scenario === "interrupted") await assert.rejects(get(url));
      else {
        const downloaded = (await get(url)).bytes;
        if (scenario === "bad-signature")
          assert.notDeepEqual(downloaded, artifact);
        else assert.deepEqual(downloaded, artifact);
      }
    }
    await writeFile(
      path.join(directory, "update-version.txt"),
      "9.8.7-update-test.2",
    );
    assert.equal(
      JSON.parse((await get(feed.endpoint)).bytes).version,
      "9.8.7-update-test.2",
    );
  },
);
