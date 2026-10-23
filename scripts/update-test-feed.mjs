// Disposable, authenticated-by-TLS/signature local update feed. No production keys.
import https from "node:https";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import path from "node:path";

const [mode, directory] = process.argv.slice(2);
if (!directory)
  throw new Error("Pass setup|serve and an owned temporary directory");
const root = path.resolve(directory);
const file = (name) => path.join(root, name);
if (mode === "setup") {
  await mkdir(root, { recursive: true });
  const openssl =
    process.platform === "win32"
      ? "C:\\Program Files\\Git\\usr\\bin\\openssl.exe"
      : "openssl";
  const run = (exe, args) => {
    const result = spawnSync(exe, args, { encoding: "utf8", stdio: "pipe" });
    // Never print key material or signing CLI output.
    if (result.status !== 0)
      throw new Error(
        `Controlled fixture command failed (${path.basename(exe)}, exit ${result.status})`,
      );
  };
  run(openssl, [
    "req",
    "-x509",
    "-newkey",
    "rsa:2048",
    "-nodes",
    "-sha256",
    "-days",
    "2",
    "-subj",
    "/CN=Atlas disposable update test CA",
    "-addext",
    "basicConstraints=critical,CA:TRUE",
    "-addext",
    "keyUsage=critical,keyCertSign,cRLSign",
    "-keyout",
    file("ca.key"),
    "-out",
    file("ca.pem"),
  ]);
  run(openssl, [
    "req",
    "-new",
    "-newkey",
    "rsa:2048",
    "-nodes",
    "-sha256",
    "-subj",
    "/CN=localhost",
    "-keyout",
    file("server.key"),
    "-out",
    file("server.csr"),
  ]);
  await writeFile(
    file("server.ext"),
    "basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost\n",
  );
  run(openssl, [
    "x509",
    "-req",
    "-in",
    file("server.csr"),
    "-CA",
    file("ca.pem"),
    "-CAkey",
    file("ca.key"),
    "-CAcreateserial",
    "-days",
    "2",
    "-sha256",
    "-extfile",
    file("server.ext"),
    "-out",
    file("server.pem"),
  ]);
  run(process.execPath, [
    "node_modules/@tauri-apps/cli/tauri.js",
    "signer",
    "generate",
    "--ci",
    "--password",
    "",
    "--write-keys",
    file("update.key"),
  ]);
  await writeFile(file("mode.txt"), "older");
} else if (mode === "serve") {
  const server = https.createServer(
    {
      key: await readFile(file("server.key")),
      cert: await readFile(file("server.pem")),
    },
    async (req, res) => {
      try {
        if (req.url === "/health") {
          res.writeHead(200);
          res.end("Atlas controlled HTTPS feed");
          return;
        }
        const scenario = (await readFile(file("mode.txt"), "utf8")).trim();
        if (req.url === "/latest.json") {
          const signature = (
            await readFile(file("update.exe.sig"), "utf8")
          ).trim();
          const platform =
            scenario === "wrong-platform" ? "darwin-aarch64" : "windows-x86_64";
          const manifest = {
            version: scenario === "older" ? "0.2.0" : "0.3.1-update-test.1",
            notes:
              "Disposable controlled update test; never a production release.",
            pub_date: "2026-10-22T00:00:00Z",
            platforms: {
              [platform]: {
                url: `https://localhost:${server.address().port}/update.exe`,
                signature,
              },
            },
          };
          res.writeHead(200, { "Content-Type": "application/json" });
          res.end(JSON.stringify(manifest));
          return;
        }
        if (req.url === "/update.exe") {
          const bytes = await readFile(file("update.exe"));
          if (scenario === "bad-signature") bytes[bytes.length - 1] ^= 1;
          res.writeHead(200, {
            "Content-Type": "application/octet-stream",
            "Content-Length": bytes.length,
          });
          if (scenario === "interrupted") {
            res.write(bytes.subarray(0, 4096));
            setTimeout(() => res.destroy(), 100);
          } else res.end(bytes);
          return;
        }
        res.writeHead(404);
        res.end();
      } catch {
        res.writeHead(503);
        res.end("Controlled artifact is not ready");
      }
    },
  );
  server.listen(0, "localhost", async () => {
    await writeFile(
      file("feed.json"),
      JSON.stringify({
        endpoint: `https://localhost:${server.address().port}/latest.json`,
      }),
    );
  });
  process.on("SIGTERM", () => server.close());
} else throw new Error("Unknown fixture mode");
