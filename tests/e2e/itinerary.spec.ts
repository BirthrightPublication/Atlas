import { test, expect } from "@playwright/test";
for (const [width, height] of [
  [2560, 1440],
  [1920, 1080],
  [1366, 768],
  [1024, 768],
  [390, 844],
]) {
  test(`v0.5 reviewed journey and model settings at ${width}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height });
    await page.goto("/");
    const rpc = async (method: string, payload: any = {}) => {
      const r = await page.request.post("/api/rpc", {
        data: { method, payload },
      });
      expect(r.ok(), await r.text()).toBeTruthy();
      return r.json();
    };
    const before = await rpc("records.list", { kind: "stay" });
    await page
      .getByRole("navigation", { name: "Main navigation" })
      .getByRole("button", { name: "Map", exact: true })
      .click();
    const workspace = page.getByTestId("journey-planner");
    await workspace.locator("summary").first().click();
    await workspace
      .getByLabel("Journey name", { exact: true })
      .fill(`Synthetic v05 journey ${width}`);
    await workspace
      .getByLabel("Destination", { exact: true })
      .first()
      .fill("Rome");
    await workspace.getByLabel("Arrival", { exact: true }).fill("2040-12-12");
    await workspace
      .getByLabel("Departure", { exact: true })
      .first()
      .fill("2041-01-11");
    await workspace
      .getByLabel("Accommodation total", { exact: false })
      .fill("900.00");
    await workspace
      .getByLabel("Accommodation total", { exact: false })
      .press("Tab");
    await workspace
      .getByRole("button", {
        name: "Compare draft with saved plan",
        exact: true,
      })
      .click();
    await expect(
      workspace.getByRole("heading", {
        name: "Hypothetical journey · no commitments changed",
      }),
    ).toBeVisible();
    expect(await rpc("records.list", { kind: "stay" })).toEqual(before);
    await page.screenshot({ path: `test-results/v050-${width}-journey.png` });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await workspace
      .getByRole("button", { name: "Save tentative journey", exact: true })
      .click();
    await expect(
      workspace.getByText(
        "Tentative journey saved. Bookings were not changed.",
      ),
    ).toBeVisible();
    await workspace
      .getByText("Propose a day-by-day schedule", { exact: true })
      .click();
    await workspace.getByLabel("Days", { exact: true }).fill("2");
    await workspace
      .getByRole("button", { name: "Generate schedule proposal", exact: true })
      .click();
    await expect(
      workspace.getByRole("button", { name: "Accept objective", exact: true }),
    ).toHaveCount(2);
    await workspace
      .getByRole("button", { name: "Accept objective", exact: true })
      .first()
      .click();
    await expect(
      workspace.getByRole("button", { name: "Accept objective", exact: true }),
    ).toHaveCount(1);
    await page.reload();
    await page
      .getByRole("navigation", { name: "Main navigation" })
      .getByRole("button", { name: "Map", exact: true })
      .click();
    await workspace.locator("summary").first().click();
    await workspace
      .getByLabel("Saved journey")
      .selectOption({ label: `Synthetic v05 journey ${width}` });
    await workspace
      .getByText("Export selected trip for offline use", { exact: true })
      .click();
    const waiting = page.waitForEvent("download");
    await workspace
      .getByRole("button", { name: "Download selected trip package" })
      .click();
    const downloaded = await waiting;
    expect(downloaded.suggestedFilename()).toBe("Atlas-selected-trip.json");
    await page
      .getByRole("navigation", { name: "Main navigation" })
      .getByRole("button", { name: "Settings", exact: true })
      .click();
    await page.getByRole("tab", { name: "Local AI", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "Download model", exact: true }),
    ).toBeDisabled();
    await expect(
      page.getByText("639,446,688 model bytes", { exact: false }),
    ).toBeVisible();
    await page.screenshot({ path: `test-results/v050-${width}-local-ai.png` });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
  });
}
