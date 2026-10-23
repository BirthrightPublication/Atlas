import { test, expect, Page } from "@playwright/test";
async function rpc(page: Page, method: string, payload: object = {}) {
  const r = await page.request.post("/api/rpc", { data: { method, payload } });
  const body = await r.json();
  expect(r.ok(), JSON.stringify(body)).toBeTruthy();
  return body;
}
const confirmation = (suffix: string) =>
  `Booking confirmation\nProvider: Synthetic v04\nReservation ID: UI-${suffix}\nProperty: Test apartment ${suffix}\nAddress: Synthetic Street, Lisbon\nCheck-in: 2028-12-12\nCheck-out: 2029-01-11\nTimezone: Europe/Lisbon\nTotal: USD 900.00\nRefrigerator: yes\nWorkspace: yes\nStatus: confirmed`;
for (const [width, height] of [
  [2560, 1440],
  [1920, 1080],
  [1366, 768],
  [1024, 768],
  [390, 844],
]) {
  test(`v0.4 import review, readiness and comparison at ${width}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height });
    await page.clock.setFixedTime(new Date("2028-12-01T12:00:00Z"));
    await page.goto("/");
    const draft = (
      await rpc(page, "imports.parse", { text: confirmation(String(width)) })
    )[0];
    await page.reload();
    await page.getByRole("button", { name: /^Import booking/ }).click();
    await expect(
      page.getByRole("heading", { name: "Booking Import Center" }),
    ).toBeVisible();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    const card = page.locator(".import-card").filter({
      has: page.getByRole("heading", {
        name: `Test apartment ${width}`,
        exact: true,
      }),
    });
    await card
      .getByRole("button", { name: "Review fields", exact: true })
      .click();
    await expect(card.getByText("Review extracted fields")).toBeVisible();
    await page.screenshot({ path: `test-results/v040-${width}-imports.png` });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await card
      .getByRole("button", { name: "Accept booking", exact: true })
      .click();
    await expect(card).toHaveCount(0);
    await page
      .getByRole("button", { name: "Close imports", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Trip Readiness", exact: true })
      .click();
    await expect(
      page.getByRole("heading", { name: "Trip Readiness", exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: /Unknown \/ unverified/ }),
    ).toBeVisible();
    await page.screenshot({ path: `test-results/v040-${width}-readiness.png` });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    for (const [suffix, amount, verified] of [
      ["A", 90000, true],
      ["B", 80000, false],
    ] as const) {
      await rpc(page, "records.save", {
        record: {
          kind: "quote",
          name: `UI offer ${width}-${suffix}`,
          checkIn: "2028-12-12",
          checkOut: "2029-01-11",
          totalMinor: amount,
          currency: "USD",
          feesComplete: verified,
          timezone: "Europe/Lisbon",
          facilities: { workspace: verified ? true : null, refrigerator: true },
          provider: "Synthetic source",
          checkedAt: "2028-11-30",
          availability: "unknown",
        },
      });
    }
    await page.reload();
    await page
      .getByRole("navigation", { name: "Main navigation" })
      .getByRole("button", { name: "Discover", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Saved places & offers", exact: true })
      .click();
    for (const suffix of ["A", "B"]) {
      await page
        .locator(".offer-card")
        .filter({
          has: page.getByRole("button", {
            name: `UI offer ${width}-${suffix}`,
            exact: true,
          }),
        })
        .getByLabel("Compare this offer")
        .check();
    }
    await page.getByRole("button", { name: "Compare", exact: true }).click();
    await page
      .getByRole("button", { name: "Compare selected offers", exact: true })
      .click();
    await expect(page.locator(".comparison-cards .import-card")).toHaveCount(2);
    await page.screenshot({ path: `test-results/v040-${width}-compare.png` });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page
      .getByRole("button", { name: "Preview budget with this offer" })
      .first()
      .click();
    await expect(
      page.getByRole("heading", {
        name: "Budget scenario · no records changed",
      }),
    ).toBeVisible();
    const stayed = await rpc(page, "records.list", { kind: "stay" });
    expect(
      stayed.filter((r: any) => r.reservationId === `UI-${width}`),
    ).toHaveLength(1);
  });
}
test("field choices, refund review, connections and navigation remain actionable", async ({
  page,
}) => {
  await page.clock.setFixedTime(new Date("2028-12-01T12:00:00Z"));
  await page.goto("/");
  const draft = (
    await rpc(page, "imports.parse", { text: confirmation("revision") })
  )[0];
  const stay = await rpc(page, "imports.accept", { id: draft.id });
  await rpc(page, "imports.parse", {
    text: "Revised reservation\nProvider: Synthetic v04\nReservation ID: UI-revision\nCheck-out: 2029-01-12\nTotal: USD 950.00",
  });
  await page.reload();
  await page.getByRole("button", { name: /^Import booking/ }).click();
  await page.getByRole("button", { name: "Changes", exact: true }).click();
  const card = page.locator(".import-card").filter({
    has: page.getByRole("heading", {
      name: "Test apartment revision",
      exact: true,
    }),
  });
  await card
    .getByRole("button", { name: "Review fields", exact: true })
    .click();
  await card
    .locator(".revision-field")
    .filter({ has: page.getByText("Total payable", { exact: true }) })
    .getByRole("checkbox")
    .uncheck();
  await card
    .getByRole("button", { name: "Accept selected changes", exact: true })
    .click();
  const updated = (await rpc(page, "records.list", { kind: "stay" })).find(
    (r: any) => r.id === stay.id,
  );
  expect(updated.totalMinor).toBe(90000);
  expect(updated.checkOut).toBe("2029-01-12");
  await page.getByRole("button", { name: "Connections", exact: true }).click();
  await page
    .getByRole("button", { name: "Manage connections", exact: true })
    .click();
  await expect(
    page.getByRole("tabpanel", { name: "Email & Integrations", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText(
      "Proton’s official Bridge requires a paid Proton Mail plan",
      { exact: false },
    ),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "3. Filters & start date", exact: true })
    .click();
  await expect(
    page.getByText("Approved sender domains · comma-separated", {
      exact: true,
    }),
  ).toBeVisible();
  await page.screenshot({ path: "test-results/v040-email-filters.png" });
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("button", { name: "Calendar", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Trip Readiness", exact: true }),
  ).toBeVisible();
});
