// Exercise real application services against a synthetic, isolated profile.
// Use seed with the previous installed host, then verify with the new host.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:net";
import { once } from "node:events";
import { readFile, writeFile, readdir } from "node:fs/promises";
import { createHash } from "node:crypto";
import { DatabaseSync } from "node:sqlite";
import path from "node:path";

const [mode, executable, dataDir, snapshotFile] = process.argv.slice(2);
if (
  !["seed", "capture", "verify"].includes(mode) ||
  !executable ||
  !dataDir ||
  !snapshotFile
)
  throw new Error(
    "Usage: node scripts/check-upgrade.mjs seed|capture|verify HOST ISOLATED_DATA_DIR SNAPSHOT_JSON",
  );
const socket = createServer();
socket.listen(0, "127.0.0.1");
await once(socket, "listening");
const port = socket.address().port;
await new Promise((resolve) => socket.close(resolve));
const host = spawn(
  path.resolve(executable),
  ["--dev", "--data-dir", path.resolve(dataDir), "--port", String(port)],
  { stdio: ["ignore", "ignore", "pipe"] },
);
let diagnostic = "";
host.stderr.on("data", (bytes) => {
  diagnostic += bytes.toString();
});
host.on("error", (error) => {
  diagnostic += error.message;
});
const origin = `http://127.0.0.1:${port}`;
const rpc = async (method, payload = {}) => {
  const response = await fetch(`${origin}/api/rpc`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ method, payload }),
  });
  const body = await response.json();
  assert.ok(response.ok, JSON.stringify(body));
  return body;
};
try {
  let ready = false;
  for (let attempt = 0; attempt < 100; attempt++) {
    if (host.exitCode !== null)
      throw new Error(`Upgrade host exited: ${diagnostic}`);
    try {
      ready = (await fetch(`${origin}/api/health`)).ok;
    } catch {
      /* starting */
    }
    if (ready) break;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  assert.ok(ready, `Upgrade host did not start: ${diagnostic}`);
  if (mode === "seed") {
    assert.equal(
      (await rpc("snapshot")).records.length,
      0,
      "Seed only an empty synthetic profile",
    );
    const stay = await rpc("records.save", {
      record: {
        kind: "stay",
        name: "Synthetic upgrade apartment",
        type: "apartment",
        address: "Synthetic upgrade address",
        timezone: "Europe/Lisbon",
        checkIn: "2026-12-12",
        checkOut: "2027-01-11",
        status: "confirmed",
        currency: "USD",
        totalMinor: 90000,
        context: "on_foot",
        provider: "Synthetic",
        reservationId: "UPGRADE-020",
        facilities: { refrigerator: true, workspace: true, internet: null },
      },
    });
    await rpc("records.save", {
      record: { ...stay, notes: "Preserve this revision and stable ID" },
      expectedVersion: stay.version,
    });
    await rpc("records.save", {
      record: {
        kind: "payment",
        name: "Synthetic upgrade prepayment",
        bookingId: stay.id,
        date: "2026-11-20",
        currency: "USD",
        amountMinor: 90000,
        status: "settled",
        type: "payment",
      },
    });
    await rpc("records.save", {
      record: {
        kind: "objective",
        name: "Synthetic upgrade objective",
        stayId: stay.id,
        category: "groceries",
        date: "2026-12-14",
        timezone: "Europe/Lisbon",
        status: "planned",
      },
    });
    await rpc("settings.save", {
      settings: {
        currency: "USD",
        context: "on_road",
        currentTimezone: "Europe/Lisbon",
        preferences: {
          name: "Synthetic retained preset",
          refrigeratorRequired: true,
          monthlyTargetMinor: 100000,
        },
        presets: [
          { name: "Synthetic reusable preset", workspacePriority: true },
        ],
        officeTimezone: "America/Los_Angeles",
        workHours: 8,
        overlapHours: 4,
      },
    });
    await rpc("imports.file", {
      filename: "synthetic-upgrade-document.txt",
      contentBase64: Buffer.from(
        "Synthetic document retained during manual installer upgrade.",
      ).toString("base64"),
    });
  }
  if (mode === "capture" || mode === "seed") {
    const conversation = await rpc("chat.create");
    await rpc("chat.send", {
      id: conversation.id,
      requestId: "synthetic-upgrade-chat",
      text: "What is my daily cost?",
      context: { date: "2026-12-14", month: "2026-12", module: "Budget" },
    });
    await rpc("notifications.list");
  }
  const snapshot = await rpc("snapshot");
  const histories = {};
  for (const record of snapshot.records)
    histories[record.id] = await rpc("records.history", { id: record.id });
  const documents = {};
  for (const name of (await readdir(path.join(dataDir, "documents"))).sort())
    documents[name] = createHash("sha256")
      .update(await readFile(path.join(dataDir, "documents", name)))
      .digest("hex");
  const december = await rpc("budget.summary", {
    month: "2026-12",
    currency: "USD",
  });
  const january = await rpc("budget.summary", {
    month: "2027-01",
    currency: "USD",
  });
  const november = await rpc("budget.summary", {
    month: "2026-11",
    currency: "USD",
  });
  assert.equal(december.accommodationMinor, 60000);
  assert.equal(january.accommodationMinor, 30000);
  assert.equal(november.cashFlowMinor, 90000);
  const db = new DatabaseSync(path.join(dataDir, "atlas.sqlite3"), {
    readOnly: true,
  });
  let schema;
  try {
    assert.equal(
      db.prepare("PRAGMA integrity_check").get().integrity_check,
      "ok",
    );
    schema = db.prepare("PRAGMA user_version").get().user_version;
  } finally {
    db.close();
  }
  const result = {
    snapshot,
    histories,
    documents,
    schema,
    accounting: [
      december.accommodationMinor,
      january.accommodationMinor,
      november.cashFlowMinor,
    ],
  };
  const baseline =
    mode === "verify" ? JSON.parse(await readFile(snapshotFile, "utf8")) : null;
  if (mode === "capture" || mode === "seed" || baseline?.panels) {
    const chats = await rpc("chat.list");
    result.panels = {
      chats: await Promise.all(
        chats.map((chat) => rpc("chat.get", { id: chat.id })),
      ),
      notifications: await rpc("notifications.list"),
    };
  }
  if (mode !== "verify")
    await writeFile(snapshotFile, JSON.stringify(result, null, 2));
  else {
    if ([3, 4].includes(baseline.schema) && schema === 5) {
      const backups = await readdir(path.join(dataDir, "backups"));
      assert.ok(
        backups.some((name) => name.startsWith(`pre-migration-v${baseline.schema}-`)),
        "Schema upgrade requires a retained pre-migration backup",
      );
      baseline.schema = 5;
    }
    assert.deepEqual(
      result,
      baseline,
      "Upgrade must retain records, IDs, versions, history, settings, documents, conversations, notifications and exact accounting; only the backed-up supported schema migration is allowed",
    );
  }
  console.log(
    `Upgrade ${mode} passed: ${snapshot.records.length} records, ${Object.keys(documents).length} documents, schema ${schema}, exact cross-month accounting and SQLite integrity.`,
  );
} finally {
  if (host.exitCode === null) {
    const stopped = once(host, "exit");
    host.kill();
    await Promise.race([
      stopped,
      new Promise((_, reject) =>
        setTimeout(
          () => reject(new Error("Owned upgrade host did not stop")),
          5000,
        ).unref(),
      ),
    ]);
  }
}
