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
  test(`neutral workspace, dock and dialogs at ${width}×${height}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height });
    await openAtlas(page);
    const navigation = page.getByRole("navigation", {
      name: "Main navigation",
    });
    await expect(navigation.getByRole("button")).toHaveCount(5);
    await expect(page.locator(".sidebar, .day-rail")).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: "Settings", exact: true }),
    ).toHaveCount(1);
    await expect(
      page.getByRole("button", { name: "Settings", exact: true }),
    ).toHaveText("");

    for (const section of ["Calendar", "Discover", "Map", "Budget"]) {
      await navigation
        .getByRole("button", { name: section, exact: true })
        .click();
      await expect(
        navigation.getByRole("button", { name: section, exact: true }),
      ).toHaveAttribute("aria-current", "page");
      await expect
        .poll(() =>
          page.locator(".workspace-content").evaluate((el) => el.scrollTop),
        )
        .toBe(0);
      await expect(navigation).toBeInViewport();
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
      expect(
        await navigation.evaluate((el) => getComputedStyle(el).backgroundColor),
      ).toBe("rgb(21, 23, 27)");
      const dock = (await navigation.boundingBox())!;
      const workspace = (await page
        .locator(".workspace-content")
        .boundingBox())!;
      expect(workspace.y + workspace.height).toBeLessThanOrEqual(dock.y + 1);
      expect(dock.y + dock.height).toBeLessThanOrEqual(height + 1);
      if (section === "Calendar") {
        const calendar = (await page.locator(".calendar-main").boundingBox())!;
        expect(calendar.width).toBeGreaterThan(width - 110);
        await expect(
          page.locator(width === 390 ? ".mobile-agenda" : ".desktop-calendar"),
        ).toBeVisible();
      }
      await page.screenshot({
        path: `test-results/v020-${width}-${section.toLowerCase()}.png`,
      });
      // Even at the end of a long module, the dock and final content remain usable.
      await page
        .locator(".workspace-content")
        .evaluate((el) => (el.scrollTop = el.scrollHeight));
      await expect(
        navigation.getByRole("button", { name: "Settings", exact: true }),
      ).toBeInViewport();
    }
    const gear = navigation.getByRole("button", {
      name: "Settings",
      exact: true,
    });
    const budget = navigation.getByRole("button", {
      name: "Budget",
      exact: true,
    });
    expect((await gear.boundingBox())!.x).toBeGreaterThan(
      (await budget.boundingBox())!.x,
    );
    await gear.click();
    const dialog = page.getByRole("dialog", { name: "Settings", exact: true });
    await expect(dialog).toBeVisible();
    const box = (await dialog.boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.y).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(width);
    expect(box.y + box.height).toBeLessThanOrEqual(height);
    await page.screenshot({ path: `test-results/v020-${width}-settings.png` });
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(gear).toBeFocused();
  });
}

test("settings keyboard flow preserves records and unrelated concurrent preferences", async ({
  page,
}) => {
  await openAtlas(page);
  const before = await rpc(page, "snapshot");
  const gear = page.getByRole("button", { name: "Settings", exact: true });
  await gear.click();
  await expect(
    page.getByRole("button", { name: "Close settings" }),
  ).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(
    page.getByRole("button", { name: "Save preferences" }),
  ).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("button", { name: "Close settings" }),
  ).toBeFocused();
  await page.getByRole("tab", { name: "General", exact: true }).focus();
  await page.keyboard.press("ArrowRight");
  await expect(
    page.getByRole("tab", { name: "Appearance", exact: true }),
  ).toBeFocused();
  await expect(page.getByText("Active theme", { exact: true })).toBeVisible();
  await page.getByRole("tab", { name: "General", exact: true }).click();
  await page
    .getByLabel("Confirmed current location")
    .fill("Synthetic interface test location");
  // A connector/background update must not be overwritten by saving General.
  await rpc(page, "settings.save", {
    settings: { uiRegressionMarker: "concurrent-setting" },
  });
  await page.getByRole("button", { name: "Save preferences" }).click();
  await expect(
    page.getByText("Preferences saved.", { exact: true }),
  ).toBeVisible();
  const after = await rpc(page, "snapshot");
  expect(after.settings.currentLocation).toBe(
    "Synthetic interface test location",
  );
  expect(after.settings.uiRegressionMarker).toBe("concurrent-setting");
  expect(after.settings.preferences).toEqual(before.settings.preferences);
  expect(after.records).toEqual(before.records);
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
    await expect(
      page.getByRole("tab", { name: category, exact: true }),
    ).toHaveAttribute("aria-selected", "true");
    await expect(page.getByRole("tabpanel")).toBeVisible();
  }
  await expect(
    page.getByText("Version 0.2.0 · Development preview", { exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.reload();
  await gear.click();
  await expect(page.getByLabel("Confirmed current location")).toHaveValue(
    "Synthetic interface test location",
  );
});

test("manual stay edits, transport and contextual details use the real services", async ({
  page,
}) => {
  await openAtlas(page);
  await page.getByRole("button", { name: "New entry", exact: true }).click();
  await page.getByRole("button", { name: "Stay", exact: true }).click();
  await page.getByLabel("Name", { exact: true }).fill("Synthetic v0.2 stay");
  await page.getByLabel("Full payable total").fill("210");
  await page.getByRole("button", { name: "Save stay", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  let stay = (await rpc(page, "records.list", { kind: "stay" })).find(
    (r: any) => r.name === "Synthetic v0.2 stay",
  );
  expect(stay.totalMinor).toBe(21000);
  await page
    .locator(".stay-band")
    .filter({ hasText: "Synthetic v0.2 stay" })
    .first()
    .click();
  await page.getByRole("button", { name: "Edit stay", exact: true }).click();
  await page
    .getByLabel("Name", { exact: true })
    .fill("Synthetic v0.2 stay edited");
  await page.getByRole("button", { name: "Save stay", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(
    (await rpc(page, "records.list", { kind: "stay" })).find(
      (r: any) => r.id === stay.id,
    ).name,
  ).toBe("Synthetic v0.2 stay edited");
  await page.getByRole("button", { name: "New entry", exact: true }).click();
  await page.getByRole("button", { name: "Transport", exact: true }).click();
  await page.getByLabel("Name", { exact: true }).fill("Synthetic v0.2 train");
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
      (r: any) => r.name === "Synthetic v0.2 train",
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
    page.getByRole("dialog", { name: "ATLAS ASSISTANT" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.reload();
  await expect(
    page
      .locator(".stay-band")
      .filter({ hasText: "Synthetic v0.2 stay edited" })
      .first(),
  ).toBeVisible();
});
