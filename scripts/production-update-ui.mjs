// Connect only to the actual installed WebView2 on an isolated Windows runner.
// The action is a DOM click on Atlas's normal Update Now button. No mocked IPC,
// test feed, private key, updater feature, or direct update_install invocation.
import { chromium, expect } from "@playwright/test";
import { spawnSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

if (process.platform !== "win32" || process.env.GITHUB_ACTIONS !== "true")
  throw Error("Disposable Windows Actions runner required.");
const [port, host, dataDir, snapshot, version] = process.argv.slice(2);
const temporary = path.resolve(process.env.RUNNER_TEMP || "");
if (
  !/^\d{4,5}$/.test(port || "") ||
  Number(port) > 65535 ||
  !/^\d+\.\d+\.\d+$/.test(version || "")
)
  throw Error("Valid loopback port and expected version required.");
for (const file of [host, dataDir, snapshot]) {
  const relative = path.relative(temporary, path.resolve(file || ""));
  if (!relative || relative.startsWith("..") || path.isAbsolute(relative))
    throw Error(
      "All installed/profile paths must be owned runner-temporary paths.",
    );
}
const reportPath = path.resolve(
  "release/windows-x86_64/production-update-smoke.json",
);
const preparation = JSON.parse(
  await readFile(
    path.resolve("release/windows-x86_64/production-preparation.json"),
    "utf8",
  ),
);
if (
  preparation.target !== version ||
  !preparation.publishedFeedVerified ||
  !preparation.bootstrapSignatureVerified ||
  !preparation.samePublicIdentity
)
  throw Error(
    "Verified real-feed/same-identity preparation is required before native UI acceptance.",
  );
const report = {
  status: "running",
  syntheticOnly: true,
  realPublishedFeed: true,
  disposableSigningCredentials: false,
  directUpdateInstallInvocation: false,
  buttonClicks: 0,
  target: version,
  progress: [],
};
let browser;
async function attach(timeout = 120000) {
  const started = Date.now();
  while (Date.now() - started < timeout) {
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, {
        timeout: 3000,
      });
      const page = browser
        .contexts()
        .flatMap((context) => context.pages())
        .find((page) =>
          /^https?:\/\/tauri\.localhost(?:\/|$)/.test(page.url()),
        );
      if (page && (await page.evaluate(() => !!window.__TAURI_INTERNALS__)))
        return page;
    } catch {
      /* WebView2 is starting/restarting. */
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  throw Error(
    "The installed native WebView2 did not expose its loopback diagnostic connection.",
  );
}
try {
  const page = await attach();
  await page
    .getByRole("button", { name: "Application updates", exact: true })
    .click();
  const panel = page.getByRole("complementary", {
    name: "Updates",
    exact: true,
  });
  await expect
    .poll(
      async () => {
        if (
          await panel
            .getByRole("heading", { name: `Version ${version}`, exact: true })
            .isVisible()
        )
          return true;
        const check = panel.getByRole("button", {
          name: "Check for updates",
          exact: true,
        });
        if (await check.isEnabled()) await check.click();
        return false;
      },
      { timeout: 90000, intervals: [1000, 5000] },
    )
    .toBe(true);
  const button = panel.getByRole("button", { name: "Update Now", exact: true });
  await expect(button).toBeEnabled();
  await page.screenshot({
    path: "release/windows-x86_64/production-before.png",
  });
  const capture = spawnSync(
    process.execPath,
    ["scripts/check-upgrade.mjs", "capture", host, dataDir, snapshot],
    { encoding: "utf8", timeout: 60000 },
  );
  if (capture.status !== 0)
    throw Error(
      "Synthetic pre-update retention snapshot failed; no click performed.",
    );
  await page.exposeFunction("__atlasObserveProductionProgress", (payload) => {
    const phases = [
      "Preparing backup",
      "Downloading",
      "Verifying signature",
      "Installing",
      "Restarting",
      "Failed",
    ];
    if (!phases.includes(payload.phase)) return;
    report.progress.push({
      phase: payload.phase,
      downloaded: payload.downloaded ?? null,
      total: payload.total ?? null,
    });
  });
  await page.evaluate(async () => {
    const api = window.__TAURI_INTERNALS__;
    await api.invoke("plugin:event|listen", {
      event: "atlas:update-progress",
      target: { kind: "Any" },
      handler: api.transformCallback((event) =>
        window.__atlasObserveProductionProgress(event.payload),
      ),
    });
  });
  report.buttonClicks = 1;
  try {
    await button.click();
  } catch (error) {
    if (!page.isClosed() && browser.isConnected()) throw error;
  }
  // Observe the old native process closing; reconnect only to its relaunched UI.
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(Error("Native application did not exit for installation.")),
      180000,
    );
    if (!browser.isConnected()) {
      clearTimeout(timeout);
      resolve();
    } else
      browser.once("disconnected", () => {
        clearTimeout(timeout);
        resolve();
      });
  });
  const restarted = await attach(180000);
  await restarted
    .getByRole("button", { name: "Application updates", exact: true })
    .click();
  await expect(
    restarted.getByRole("heading", { name: `Atlas ${version}`, exact: true }),
  ).toBeVisible({ timeout: 90000 });
  await expect
    .poll(
      async () => {
        if (
          await restarted
            .getByText(
              "Atlas is up to date on this build’s signed update feed.",
              { exact: true },
            )
            .isVisible()
        )
          return true;
        const check = restarted.getByRole("button", {
          name: "Check for updates",
          exact: true,
        });
        if (await check.isEnabled()) await check.click();
        return false;
      },
      { timeout: 90000, intervals: [1000, 5000] },
    )
    .toBe(true);
  await expect(
    restarted.getByRole("button", { name: "Update Now", exact: true }),
  ).toHaveCount(0);
  await restarted.screenshot({
    path: "release/windows-x86_64/production-after.png",
  });
  if (
    !report.progress.some((p) => p.phase === "Downloading" && p.downloaded > 0)
  )
    throw Error("No real download progress was observed.");
  report.status = "ui_passed";
  report.nativeRestart = true;
  report.installedVersion = version;
  report.retentionSnapshotCaptured = true;
  const baseline = JSON.parse(await readFile(snapshot, "utf8"));
  report.syntheticRecords = baseline.snapshot.records.length;
} catch (error) {
  report.status = "failed";
  report.failure = String(error);
  throw error;
} finally {
  await writeFile(reportPath, JSON.stringify(report, null, 2));
  // The PowerShell owner closes only installed processes under its test root.
}
process.exit(0);
