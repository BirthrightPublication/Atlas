import { test, expect, Page } from "@playwright/test";

async function rpc(page: Page, method: string, payload: object = {}) {
  const response = await page.request.post("/api/rpc", {
    data: { method, payload },
  });
  const body = await response.json();
  expect(response.ok(), JSON.stringify(body)).toBeTruthy();
  return body;
}
async function openAtlas(page: Page) {
  await page.clock.setFixedTime(new Date("2026-12-01T12:00:00Z"));
  await page.goto("/");
  await expect(page.locator("[data-connection=ready]")).toBeVisible();
}

for (const [width, height] of [
  [2560, 1440],
  [1920, 1080],
  [1366, 768],
  [1024, 768],
  [390, 844],
]) {
  test(`v0.3 page navigation, shared panels and neutral theme at ${width}×${height}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height });
    await openAtlas(page);
    const dock = page.getByRole("navigation", { name: "Main navigation" });
    await expect(dock.getByRole("button")).toHaveCount(5);
    await expect(page.locator(".sidebar,.day-rail")).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: "Settings", exact: true }),
    ).toHaveText("");
    for (const section of [
      "Calendar",
      "Discover",
      "Map",
      "Budget",
      "Settings",
    ]) {
      await dock.getByRole("button", { name: section, exact: true }).click();
      await expect(
        dock.getByRole("button", { name: section, exact: true }),
      ).toHaveAttribute("aria-current", "page");
      await expect(page.getByRole("dialog")).toHaveCount(0);
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
      ).toBe(true);
      expect(
        await page
          .locator("body")
          .evaluate((el) => getComputedStyle(el).backgroundColor),
      ).toBe("rgb(11, 12, 14)");
      const d = (await dock.boundingBox())!,
        w = (await page.locator(".workspace-content").boundingBox())!;
      expect(w.y + w.height).toBeLessThanOrEqual(d.y + 1);
      if (section === "Settings")
        await expect(
          page.getByRole("tab", { name: "General", exact: true }),
        ).toBeVisible();
      await page.screenshot({
        path: `test-results/v030-${width}-${section.toLowerCase()}.png`,
      });
    }
    const assistant = page.getByRole("button", {
      name: "Atlas Assistant",
      exact: true,
    });
    await assistant.click();
    const panel = page.getByRole("complementary", {
      name: "Atlas Assistant",
      exact: true,
    });
    await expect(panel).toBeVisible();
    await expect(page.locator(".main-shell")).not.toHaveAttribute("inert", "");
    await page.getByLabel("Ask Atlas").fill("An unsent planning question");
    await dock.getByRole("button", { name: "Budget", exact: true }).click();
    await expect(panel).toBeVisible();
    await expect(
      page.getByRole("button", {
        name: "What is my estimated daily cost?",
        exact: true,
      }),
    ).toBeVisible();
    await page.screenshot({ path: `test-results/v030-${width}-assistant.png` });
    await page
      .getByRole("button", { name: "Notifications", exact: true })
      .click();
    await expect(
      page.getByRole("complementary", { name: "Notifications", exact: true }),
    ).toBeVisible();
    await page
      .getByRole("button", { name: "Application updates", exact: true })
      .click();
    await expect(
      page.getByRole("complementary", { name: "Updates", exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Published release history" }),
    ).toBeVisible();
    await page.screenshot({ path: `test-results/v030-${width}-updates.png` });
    await assistant.click();
    await expect(page.getByLabel("Ask Atlas")).toHaveValue(
      "An unsent planning question",
    );
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await assistant.click();
    await assistant.click();
    await expect(panel).toBeHidden();
    await assistant.click();
    await page
      .getByRole("button", { name: "Close panel", exact: true })
      .click();
    await expect(panel).toBeHidden();
    if (width === 1366) {
      await dock.getByRole("button", { name: "Calendar", exact: true }).click();
      await page.evaluate(() => document.fonts.ready);
      const cdp = await page.context().newCDPSession(page);
      await cdp.send("DOM.enable");
      const { root } = await cdp.send("DOM.getDocument");
      const { nodeId } = await cdp.send("DOM.querySelector", {
        nodeId: root.nodeId,
        selector: ".day-number",
      });
      await cdp.send("CSS.enable");
      const fonts = await cdp.send("CSS.getPlatformFontsForNode", { nodeId });
      expect(
        fonts.fonts.some((font: any) =>
          /Times|Liberation Serif/.test(font.familyName),
        ),
      ).toBe(true);
    }
  });
}

test("Settings save/discard navigation preserves records and concurrent settings", async ({
  page,
}) => {
  await openAtlas(page);
  const before = await rpc(page, "snapshot");
  const dock = page.getByRole("navigation", { name: "Main navigation" });
  await dock.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("tab", { name: "General", exact: true }).focus();
  await page.keyboard.press("ArrowRight");
  await expect(
    page.getByRole("tab", { name: "Appearance", exact: true }),
  ).toBeFocused();
  await page.getByRole("tab", { name: "General", exact: true }).click();
  await page
    .getByLabel("Confirmed current location")
    .fill("Synthetic retained v0.3 preference");
  await rpc(page, "settings.save", {
    settings: { uiV03Concurrent: "retained" },
  });
  await dock.getByRole("button", { name: "Calendar", exact: true }).click();
  await expect(
    page.getByRole("alert", { name: "Unsaved settings" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Keep editing", exact: true }).click();
  await page
    .getByRole("button", { name: "Atlas Assistant", exact: true })
    .click();
  await expect(page.getByLabel("Confirmed current location")).toHaveValue(
    "Synthetic retained v0.3 preference",
  );
  await dock.getByRole("button", { name: "Budget", exact: true }).click();
  await page
    .getByRole("button", { name: "Save and continue", exact: true })
    .click();
  await expect(
    dock.getByRole("button", { name: "Budget", exact: true }),
  ).toHaveAttribute("aria-current", "page");
  const saved = await rpc(page, "snapshot");
  expect(saved.settings.currentLocation).toBe(
    "Synthetic retained v0.3 preference",
  );
  expect(saved.settings.uiV03Concurrent).toBe("retained");
  expect(saved.records).toEqual(before.records);
  await dock.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByLabel("Confirmed current location").fill("Discard this edit");
  await dock.getByRole("button", { name: "Map", exact: true }).click();
  await page
    .getByRole("button", { name: "Discard and continue", exact: true })
    .click();
  await page.reload();
  await dock.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(page.getByLabel("Confirmed current location")).toHaveValue(
    "Synthetic retained v0.3 preference",
  );
  for (const category of [
    "Travel Preferences",
    "Work Schedule",
    "Budget Defaults",
    "Notifications",
    "Email & Integrations",
    "Privacy & Data",
    "Application Updates",
    "About Atlas",
  ]) {
    await page.getByRole("tab", { name: category, exact: true }).click();
    await expect(page.getByRole("tabpanel")).toBeVisible();
  }
  await expect(
    page.getByText("Version 0.3.0 · Development preview", { exact: true }),
  ).toBeVisible();
});

test("Assistant sessions persist, reopen, delete and use the selected budget month", async ({
  page,
}) => {
  await openAtlas(page);
  await page.getByRole("button", { name: "Budget", exact: true }).click();
  await page
    .getByRole("button", { name: "Atlas Assistant", exact: true })
    .click();
  await page
    .getByRole("button", { name: "New conversation", exact: true })
    .click();
  await page.getByLabel("Ask Atlas").fill("What is my estimated daily cost?");
  await page.getByLabel("Ask Atlas").press("Enter");
  await expect(page.locator(".chat-messages article.assistant")).toContainText(
    "2026-12",
  );
  await page
    .getByLabel("Conversation title")
    .fill("Synthetic persistent v0.3 chat");
  await page.getByLabel("Conversation title").press("Tab");
  await expect
    .poll(async () =>
      (await rpc(page, "chat.list")).some(
        (c: any) => c.title === "Synthetic persistent v0.3 chat",
      ),
    )
    .toBe(true);
  await page.reload();
  await page
    .getByRole("button", { name: "Atlas Assistant", exact: true })
    .click();
  await expect(page.getByLabel("Conversation title")).toHaveValue(
    "Synthetic persistent v0.3 chat",
  );
  await expect(page.locator(".chat-messages article")).toHaveCount(2);
  await page
    .getByRole("button", { name: "New conversation", exact: true })
    .click();
  await expect(page.locator(".chat-messages article")).toHaveCount(0);
  await page.locator(".recent-chats summary").click();
  await page
    .getByRole("button", { name: /Synthetic persistent v0.3 chat/ })
    .click();
  await expect(page.locator(".chat-messages article")).toHaveCount(2);
  await page.screenshot({ path: "test-results/v030-conversation.png" });
  await page
    .getByRole("button", { name: "Delete conversation", exact: true })
    .click();
  await page.getByRole("button", { name: "Delete chat", exact: true }).click();
  await expect
    .poll(async () =>
      (await rpc(page, "chat.list")).some(
        (c: any) => c.title === "Synthetic persistent v0.3 chat",
      ),
    )
    .toBe(false);
});

test("Notification targets select the booking date and handle missing records", async ({
  page,
}) => {
  await openAtlas(page);
  const stay = await rpc(page, "records.save", {
    record: {
      kind: "stay",
      name: "Synthetic notification stay",
      checkIn: "2026-12-02",
      checkOut: "2026-12-05",
      timezone: "UTC",
      status: "confirmed",
      currency: "USD",
      totalMinor: 30000,
    },
  });
  await rpc(page, "notifications.list", { now: "2026-12-01T12:00:00Z" });
  await page.reload();
  await page
    .getByRole("button", { name: "Notifications", exact: true })
    .click();
  const notification = page.getByRole("button", {
    name: /^Checkout: Synthetic notification stay/,
  });
  await notification.click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.locator(".day-cell.selected")).toHaveAttribute(
    "aria-label",
    "Select 2026-12-05",
  );
  await page.keyboard.press("Escape");
  const stored = await rpc(page, "notifications.list");
  expect(
    stored.items.find(
      (n: any) => n.type === "checkOut" && n.target.recordId === stay.id,
    ).readAt,
  ).toBeTruthy();
  await rpc(page, "records.delete", {
    id: stay.id,
    expectedVersion: stay.version,
  });
  await page.reload();
  await page
    .getByRole("button", { name: "Notifications", exact: true })
    .click();
  await notification.click();
  await expect(page.getByRole("status")).toContainText(
    "deleted or is no longer available",
  );
  await page
    .getByRole("button", { name: "Mark All as Read", exact: true })
    .click();
  await expect
    .poll(async () => (await rpc(page, "notifications.list")).unread)
    .toBe(0);
});

test("manual stay edits, transport and contextual details use the real services", async ({
  page,
}) => {
  await openAtlas(page);
  await page.getByRole("button", { name: "New entry", exact: true }).click();
  await page.getByRole("button", { name: "Stay", exact: true }).click();
  await page.getByLabel("Name", { exact: true }).fill("Synthetic v0.3 stay");
  await page.getByLabel("Full payable total").fill("210");
  await page.getByLabel("Booking status").selectOption("confirmed");
  await page.getByRole("button", { name: "Save stay", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  let stay = (await rpc(page, "records.list", { kind: "stay" })).find(
    (r: any) => r.name === "Synthetic v0.3 stay",
  );
  expect(stay.totalMinor).toBe(21000);
  await page
    .locator(".stay-band")
    .filter({ hasText: "Synthetic v0.3 stay" })
    .first()
    .click();
  await page.getByRole("button", { name: "Edit stay", exact: true }).click();
  await page
    .getByLabel("Name", { exact: true })
    .fill("Synthetic v0.3 stay edited");
  await page.getByRole("button", { name: "Save stay", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(
    (await rpc(page, "records.list", { kind: "stay" })).find(
      (r: any) => r.id === stay.id,
    ).name,
  ).toBe("Synthetic v0.3 stay edited");
  await page.getByRole("button", { name: "New entry", exact: true }).click();
  await page.getByRole("button", { name: "Transport", exact: true }).click();
  await page.getByLabel("Name", { exact: true }).fill("Synthetic v0.3 train");
  await page.getByLabel("Origin", { exact: true }).fill("Synthetic origin");
  await page
    .getByLabel("Destination", { exact: true })
    .fill("Synthetic destination");
  await page.getByLabel("Full payable total").fill("12.34");
  await page
    .getByRole("button", { name: "Save transport", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(
    (await rpc(page, "records.list", { kind: "transport" })).find(
      (r: any) => r.name === "Synthetic v0.3 train",
    ).totalMinor,
  ).toBe(1234);
  await page.getByText("Clocks & work", { exact: true }).click();
  await expect(
    page.getByText("EMPLOYER OFFICE", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Protect a work block" }),
  ).toBeVisible();
  await page.getByText("Daily allowance", { exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Open budget", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Atlas Assistant", exact: true })
    .click();
  await expect(
    page.getByRole("complementary", { name: "Atlas Assistant", exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.reload();
  await expect(
    page
      .locator(".stay-band")
      .filter({ hasText: "Synthetic v0.3 stay edited" })
      .first(),
  ).toBeVisible();
});

test("duplicate stays, linked documents and hypothetical transport leave originals intact", async ({
  page,
}) => {
  await openAtlas(page);
  const original = await rpc(page, "records.save", {
    record: {
      kind: "stay",
      name: "Synthetic duplicate source",
      checkIn: "2026-12-02",
      checkOut: "2026-12-06",
      timezone: "UTC",
      type: "apartment",
      status: "confirmed",
      currency: "USD",
      totalMinor: 12000,
    },
  });
  await page.reload();
  await page
    .locator(".stay-band")
    .filter({ hasText: original.name })
    .first()
    .click();
  await page
    .getByRole("button", { name: "Duplicate as draft", exact: true })
    .click();
  await expect(page.getByLabel("Booking status")).toHaveValue("tentative");
  await page
    .getByLabel("Name", { exact: true })
    .fill("Synthetic duplicate draft");
  await page
    .getByLabel("Attach booking document")
    .setInputFiles({
      name: "synthetic-ticket.txt",
      mimeType: "text/plain",
      buffer: Buffer.from(
        "Synthetic attachment for the v0.3 duplicate acceptance test.",
      ),
    });
  await expect(page.getByText(/1 linked documents/)).toBeVisible();
  await page.getByRole("button", { name: "Save stay", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  const stays = await rpc(page, "records.list", { kind: "stay" });
  expect(stays.find((r: any) => r.id === original.id)).toEqual(original);
  const copy = stays.find((r: any) => r.name === "Synthetic duplicate draft");
  expect(copy.id).not.toBe(original.id);
  expect(copy.status).toBe("tentative");
  expect(copy.documentIds).toHaveLength(1);
  const train = await rpc(page, "records.save", {
    record: {
      kind: "transport",
      name: "Synthetic scenario transport",
      mode: "train",
      departure: "2026-12-08T12:00:00Z",
      arrival: "2026-12-08T14:00:00Z",
      departureTimezone: "UTC",
      arrivalTimezone: "UTC",
      status: "tentative",
      currency: "USD",
      totalMinor: 1234,
    },
  });
  await page.reload();
  const before = await rpc(page, "snapshot");
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Budget", exact: true })
    .click();
  await page.getByText("Explore a transport scenario", { exact: true }).click();
  await page.getByLabel("Scenario transport").selectOption(train.id);
  await page.getByLabel("Hypothetical total (USD)").fill("62.00");
  await expect(
    page.getByText(/This scenario includes the selected transport/),
  ).toContainText("31 days");
  const exact = await rpc(page, "planning.what_if", {
    transportId: train.id,
    totalMinor: 6200,
    month: "2026-12",
  });
  expect(exact.after.forecastMinor - exact.before.forecastMinor).toBe(6200);
  expect(await rpc(page, "snapshot")).toEqual(before);
  await page.screenshot({ path: "test-results/v030-budget-scenario.png" });
});
