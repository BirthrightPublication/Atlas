import { test, expect, Page } from '@playwright/test';

async function rpc(page: Page, method: string, payload: object = {}) {
  const response = await page.request.post('/api/rpc', { data: { method, payload } });
  const body = await response.json();
  expect(response.ok(), JSON.stringify(body)).toBeTruthy();
  return body;
}

test('booking review, calendar, payments, revisions, and restart persistence', async ({ page }) => {
  await page.clock.setFixedTime(new Date('2026-12-01T12:00:00Z'));
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('/');
  await expect(page.locator('[data-connection=ready]')).toBeVisible();
  await page.getByRole('button', { name: /^Import booking/ }).click();
  await page.getByRole('button', { name: 'Try a synthetic sample confirmation' }).click();
  await page.getByRole('button', { name: 'Create review drafts' }).click();
  await expect(page.getByRole('heading', { name: 'Riverside apartment' })).toBeVisible();
  await page.getByRole('button', { name: 'Review fields' }).click();
  await expect(page.getByLabel('Full payable total')).toHaveValue('900');
  await expect(page.getByLabel('Check-out (exclusive)')).toHaveValue('2027-01-11');
  await expect(page.getByLabel('Local timezone')).toHaveValue('Europe/Lisbon');
  await page.getByRole('button', { name: 'Accept reviewed booking' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  const [stay] = await rpc(page, 'records.list', { kind: 'stay' });
  expect(stay.name).toBe('Riverside apartment');
  expect(stay.facilities.workspace).toBe(true);
  await expect(page.getByText('Riverside apartment').first()).toBeVisible();
  await page.getByRole('button', { name: 'On Road', exact: true }).click();
  await expect(page.getByText('Riverside apartment').first()).toBeVisible();
  expect((await rpc(page, 'records.list', { kind: 'stay' }))[0].context).toBe('on_foot');

  await rpc(page, 'records.save', { record: { kind: 'payment', bookingId: stay.id, date: '2026-11-20', currency: 'USD', amountMinor: 90000, status: 'settled', type: 'payment' } });
  await page.getByRole('button', { name: 'Budget', exact: true }).click();
  await expect(page.getByText('$600.00', { exact: true }).first()).toBeVisible();
  await expect(page.getByText(/20 nights this month/)).toBeVisible();
  await page.getByRole('button', { name: 'Next month', exact: true }).click();
  await expect(page.getByText('$300.00', { exact: true }).first()).toBeVisible();
  const november = await rpc(page, 'budget.summary', { month: '2026-11', currency: 'USD' });
  expect(november.cashFlowMinor).toBe(90000);
  expect(november.accommodationMinor).toBe(0);

  await page.reload();
  await expect(page.getByText('Riverside apartment').first()).toBeVisible();
  await page.getByRole('button', { name: /^Import booking/ }).click();
  await page.getByRole('button', { name: 'Try a synthetic sample confirmation' }).click();
  const field = page.getByLabel('Booking text or saved URL');
  await field.fill((await field.inputValue()).replace('USD 900.00', 'USD 930.00') + '\nRevised confirmation');
  await page.getByRole('button', { name: 'Create review drafts' }).click();
  await page.getByRole('button', { name: 'Review fields' }).click();
  await page.getByRole('button', { name: 'Accept reviewed booking' }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  const stays = await rpc(page, 'records.list', { kind: 'stay' });
  expect(stays).toHaveLength(1);
  expect(stays[0].id).toBe(stay.id);
  expect(stays[0].totalMinor).toBe(93000);
  expect(await rpc(page, 'records.history', { id: stay.id })).toHaveLength(2);
  expect(errors).toEqual([]);
});

test('manual objective and real reminder remain after reload', async ({ page }) => {
  await page.clock.setFixedTime(new Date('2026-12-01T12:00:00Z'));
  await page.goto('/');
  await expect(page.locator('[data-connection=ready]')).toBeVisible();
  await page.getByRole('button', { name: 'New entry', exact: true }).click();
  await page.getByRole('button', { name: 'Objective', exact: true }).click();
  await page.getByLabel('Name', { exact: true }).fill('Synthetic grocery stop');
  await page.getByRole('button', { name: 'Save objective', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.getByRole('button', { name: 'New entry', exact: true }).click();
  await page.getByRole('button', { name: 'Reminder', exact: true }).click();
  await page.getByLabel('Reminder', { exact: true }).fill('Synthetic checkout reminder');
  await page.getByRole('button', { name: 'Save reminder', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.reload();
  const reminders = await rpc(page, 'records.list', { kind: 'reminder' });
  expect(reminders.some((r: any) => r.name === 'Synthetic checkout reminder')).toBe(true);
  const objectives = await rpc(page, 'records.list', { kind: 'objective' });
  expect(objectives.some((r: any) => r.name === 'Synthetic grocery stop')).toBe(true);
  await page.getByRole('button', { name: 'Notifications', exact: true }).click();
  await expect(page.getByText('Synthetic checkout reminder', { exact: true })).toBeVisible();
});
