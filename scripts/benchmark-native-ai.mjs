// Explicit synthetic evaluation via the real packaged Rust service; never bundle weights.
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, writeFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createServer } from "node:net";
if (!process.argv.includes("--approve-model-download"))
  throw Error(
    "Approve the 639,446,688-byte model plus pinned runtime download explicitly.",
  );
const dir = await mkdtemp(path.join(tmpdir(), "atlas-native-ai-"));
const socket = createServer();
socket.listen(0, "127.0.0.1");
await once(socket, "listening");
const port = socket.address().port;
await new Promise((resolve) => socket.close(resolve));
const executable =
  process.argv.find((v) => v.startsWith("--host="))?.slice(7) ||
  path.resolve(
    "target/debug/atlas-host" + (process.platform === "win32" ? ".exe" : ""),
  );
const host = spawn(
  executable,
  ["--dev", "--data-dir", dir, "--port", String(port)],
  { stdio: "ignore" },
);
const rpc = async (method, payload = {}, timeout = 420000) => {
  const r = await fetch(`http://127.0.0.1:${port}/api/rpc`, {
    signal: AbortSignal.timeout(timeout),
      method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ method, payload }),
  });
  const b = await r.json();
  if (!r.ok) throw Error(b.error);
  return b;
};
const report = {
  model: "Qwen3-0.6B-Q8_0",
  platform: `${process.platform}-${process.arch}`,
  nativeInstaller: true,
  syntheticOnly: true,
  productionSigningConfigured: false,
};
try {
  for (let i = 0; i < 100; i++) {
    try {
      await rpc("ai.status");
      break;
    } catch {
      await new Promise((r) => setTimeout(r, 100));
    }
  }
  await rpc("ai.install", { model: "0.6b", approveDownload: true });
  report.verification = await rpc("ai.verify", { model: "0.6b" });
  await rpc("ai.enable", { model: "0.6b" });
  const cold = performance.now();
  report.cold = await rpc("ai.test");
  report.coldWallMs = Math.round(performance.now() - cold);
  const warm = performance.now();
  report.warm = await rpc("ai.test");
  report.warmWallMs = Math.round(performance.now() - warm);
  await rpc("ai.cancel");
  report.stopped = await rpc("ai.status");
  report.status = "passed";
} catch (e) {
  report.status = "blocked";
  report.reason = String(e);
} finally {
  try { await rpc("ai.cancel", {}, 3000); await rpc("ai.install_cancel", {}, 3000); } catch { /* host may already be stopped */ }
  host.kill();
  await Promise.race([
    once(host, "exit"),
    new Promise((resolve) => setTimeout(resolve, 5000)),
  ]);
}
await mkdir("release", { recursive: true });
await writeFile(
  "release/ai-installation-benchmark.json",
  JSON.stringify(report, null, 2) + "\n",
);
console.log(
  JSON.stringify({
    status: report.status,
    reason: report.reason,
    coldWallMs: report.coldWallMs,
    warmWallMs: report.warmWallMs,
  }),
);
