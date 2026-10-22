import { test, expect } from '@playwright/test'
import { spawn, type ChildProcess } from 'node:child_process'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve, join } from 'node:path'

// Browser secure-context/offline tests use the standards-defined localhost
// exception. They do not assert that private HTTPS or physical iPhone behavior
// was exercised. Production authorization is covered by atlas-host Rust tests.
const origin = 'http://127.0.0.1:47833'
let host: ChildProcess
let dataDir: string
let admin: string
let objective: Record<string, unknown>
let document: Record<string, unknown>
const date = new Date().toISOString().slice(0, 10)

async function rpc(method: string, payload: unknown = {}) {
  const response = await fetch(`${origin}/api/rpc`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ method, payload }) })
  const body = await response.json()
  if (!response.ok) throw new Error(`${method}: ${JSON.stringify(body)}`)
  return body
}
async function control(action: string, device_id?: string) {
  const response = await fetch(`${origin}/api/local/control`, { method: 'POST', headers: { 'Content-Type': 'application/json', 'x-atlas-admin': admin }, body: JSON.stringify({ action, device_id }) })
  expect(response.ok).toBeTruthy()
  return response.json()
}

test.describe('paired phone companion on localhost secure context', () => {
  test.describe.configure({ mode: 'serial' })
  test.use({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, timezoneId: 'UTC', actionTimeout: 10_000 })
  test.beforeAll(async () => {
    let occupied = false
    try { occupied = (await fetch(`${origin}/api/health`)).ok } catch { /* no listener */ }
    if (occupied) throw new Error('Companion test port 47833 is already in use; stop the previous test host before retrying.')
    dataDir = await mkdtemp(join(tmpdir(), 'atlas-companion-browser-'))
    const executable = process.env.ATLAS_TEST_HOST ?? resolve('target/debug', process.platform === 'win32' ? 'atlas-host.exe' : 'atlas-host')
    host = spawn(executable, ['--dev', '--data-dir', dataDir, '--static-dir', resolve('dist'), '--port', '47833'], { stdio: ['ignore', 'pipe', 'pipe'] })
    let startupError = ''
    host.stderr?.on('data', data => { startupError += String(data) })
    host.on('error', error => { startupError += error.message })
    await expect.poll(async () => {
      if (host.exitCode !== null) throw new Error(`Companion host exited: ${startupError}`)
      try { return (await fetch(`${origin}/api/health`)).status } catch { return 0 }
    }, { timeout: 20_000, message: 'Start the separately isolated companion host' }).toBe(200)
    admin = await readFile(join(dataDir, 'companion-admin-token'), 'utf8')
    const checkout = new Date(`${date}T12:00:00Z`); checkout.setUTCDate(checkout.getUTCDate() + 3)
    await rpc('records.save', { record: { kind: 'stay', name: 'Synthetic garden apartment', type: 'apartment', address: 'Synthetic Gardens', timezone: 'UTC', checkIn: date, checkOut: checkout.toISOString().slice(0, 10), checkInTime: '15:00', checkOutTime: '11:00', context: 'on_foot', status: 'confirmed', currency: 'USD', totalMinor: 12000, provider: 'Synthetic', reservationId: 'PWA-TEST', facilities: { refrigerator: true, workspace: true, kitchen: null, internet: null } } })
    objective = await rpc('records.save', { record: { kind: 'objective', name: 'Synthetic market walk', category: 'groceries', date, timezone: 'UTC', status: 'planned' } })
    await rpc('records.save', { record: { kind: 'budget', month: date.slice(0, 7), currency: 'USD', incomeMinor: 200000, openingBalanceMinor: 100000, reserveMinor: 30000, allowanceMinor: 60000, recurringExpenses: [] } })
    const imported = await rpc('imports.file', { filename: 'synthetic-ticket.txt', contentBase64: Buffer.from('Synthetic travel document. No private information.').toString('base64') })
    document = imported.document ?? (await rpc('records.list', { kind: 'document' }))[0]
    document = await rpc('records.save', { record: { ...document, companionAvailable: true }, expectedVersion: document.version })
  })
  test.afterAll(async () => {
    if (host && host.exitCode === null) {
      host.kill('SIGTERM')
      await new Promise<void>(resolve => { host.once('exit', () => resolve()); setTimeout(resolve, 3000) })
    }
    if (dataDir) await rm(dataDir, { recursive: true, force: true })
  })

  test('pairs, keeps a document, survives offline reload, and syncs an expense exactly once', async ({ page, context }) => {
    const pairing = await control('pair')
    await page.goto(`${origin}/companion`)
    await expect(page.getByRole('heading', { name: 'A little closer to everywhere.' })).toBeVisible()
    await page.getByLabel('This device').fill('Synthetic iPhone browser')
    await page.getByLabel('Desktop pairing code').fill(pairing.code)
    await page.getByRole('button', { name: 'Pair this device' }).click()
    await expect(page.getByRole('heading', { name: 'Synthetic garden apartment' })).toBeVisible()
    await expect(page.getByText('Desktop connected', { exact: true })).toBeVisible()
    await page.evaluate(async () => { await navigator.serviceWorker.ready })
    await page.getByRole('button', { name: 'Documents', exact: true }).click()
    await page.getByRole('button', { name: 'Keep offline', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Open', exact: true })).toBeVisible()

    await context.setOffline(true)
    await page.reload()
    await expect(page.getByRole('heading', { name: 'Synthetic garden apartment' })).toBeVisible()
    await page.getByRole('button', { name: 'Documents', exact: true }).click()
    await expect(page.getByRole('button', { name: 'Open', exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'Budget', exact: true }).click()
    await page.getByLabel('What was it?').fill('Synthetic offline groceries')
    await page.getByLabel('Amount', { exact: true }).fill('12.34')
    await page.getByRole('button', { name: 'Save expense' }).click()
    await expect(page.getByText('Synthetic offline groceries', { exact: true })).toBeVisible()
    await expect(page.getByText('1 pending', { exact: true })).toBeVisible()
    await page.reload()
    await page.getByRole('button', { name: 'Budget', exact: true }).click()
    await expect(page.getByText('Synthetic offline groceries', { exact: true })).toBeVisible()
    await context.setOffline(false)
    await page.getByRole('button', { name: 'Synchronize with desktop' }).click()
    await expect(page.getByText('Desktop connected', { exact: true })).toBeVisible()
    await expect(page.getByText('1 pending', { exact: true })).toHaveCount(0)
    await page.getByRole('button', { name: 'Synchronize with desktop' }).click()
    await expect.poll(async () => (await rpc('records.list', { kind: 'expense' })).filter((record: Record<string, unknown>) => record.name === 'Synthetic offline groceries').length).toBe(1)
    const expense = (await rpc('records.list', { kind: 'expense' })).find((record: Record<string, unknown>) => record.name === 'Synthetic offline groceries')
    expect(expense.amountMinor).toBe(1234)
    await page.screenshot({ path: 'test-results/companion-budget-mobile.png', fullPage: true })
  })

  test('preserves a queued objective conflict for explicit owner resolution', async ({ page, context }) => {
    const pairing = await control('pair')
    await page.goto(`${origin}/companion`)
    await page.getByLabel('This device').fill('Synthetic conflict phone')
    await page.getByLabel('Desktop pairing code').fill(pairing.code)
    await page.getByRole('button', { name: 'Pair this device' }).click()
    await expect(page.getByRole('button', { name: 'Complete Synthetic market walk' })).toBeVisible()
    await context.setOffline(true)
    await page.getByRole('button', { name: 'Complete Synthetic market walk' }).click()
    objective = await rpc('records.save', { record: { ...objective, notes: 'A synthetic desktop change while phone was offline' }, expectedVersion: objective.version })
    await context.setOffline(false)
    await page.getByRole('button', { name: 'Synchronize with desktop' }).click()
    await page.getByRole('button', { name: 'Device', exact: true }).click()
    await expect(page.getByText('The desktop record changed after this phone last synchronized.', { exact: false })).toBeVisible()
    await page.getByRole('button', { name: 'Discard local change' }).click()
    await expect(page.getByText('Nothing waiting. A little less to carry.')).toBeVisible()
    const records = await rpc('records.list', { kind: 'objective' })
    expect(records.find((record: Record<string, unknown>) => record.id === objective.id).status).toBe('planned')
  })
})
