import { useEffect, useState } from 'react'
import { companionRpc, keepDocument, pairDevice, queueRecord, requestPersistentStorage, syncNow } from './client'
import {
  cachedDocuments, clearPhoneData, localSnapshot, queueChanges, readState, removeChange,
  removeDocument, saveChange, saveState, type AtlasRecord, type CachedDocument,
  type CompanionIdentity, type QueuedChange, type Snapshot,
} from './storage'
import './companion.css'

const today = () => new Intl.DateTimeFormat('en-CA', { year: 'numeric', month: '2-digit', day: '2-digit' }).format(new Date())
const text = (record: AtlasRecord, field: string) => typeof record[field] === 'string' ? String(record[field]) : ''
function instantDate(instant: string, timezone: string) {
  try {
    const parts = new Intl.DateTimeFormat('en-CA', { timeZone: timezone, year: 'numeric', month: '2-digit', day: '2-digit' }).formatToParts(new Date(instant))
    return ['year', 'month', 'day'].map(type => parts.find(part => part.type === type)?.value).join('-')
  } catch { return instant.slice(0, 10) }
}
function instantLabel(instant: string, timezone: string) {
  try { return new Intl.DateTimeFormat(undefined, { timeZone: timezone, month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', timeZoneName: 'short' }).format(new Date(instant)) }
  catch { return `${instant} · timezone unavailable` }
}
const exponent = (currency: string) => new Intl.NumberFormat('en', { style: 'currency', currency }).resolvedOptions().maximumFractionDigits ?? 2
function money(value: unknown, currency = 'USD') {
  return new Intl.NumberFormat(undefined, { style: 'currency', currency }).format(Number(value ?? 0) / 10 ** exponent(currency))
}
function minorUnits(value: string, currency: string) {
  const decimals = exponent(currency)
  if (!new RegExp(decimals === 0 ? '^\\d+$' : `^\\d+(?:\\.\\d{1,${decimals}})?$`).test(value) || (decimals === 0 && value.includes('.'))) throw new Error(`Enter an amount with at most ${decimals} decimal places.`)
  const [whole, fraction = ''] = value.split('.')
  const units = BigInt(whole) * 10n ** BigInt(decimals) + BigInt(fraction.padEnd(decimals, '0') || '0')
  if (units > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('The amount is too large.')
  return Number(units)
}
function download(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a'); anchor.href = url; anchor.download = filename; anchor.click()
  window.setTimeout(() => URL.revokeObjectURL(url), 30_000)
}

export function CompanionApp() {
  const [identity, setIdentity] = useState<CompanionIdentity>()
  const [snapshot, setSnapshot] = useState<Snapshot>({ records: [], settings: {} })
  const [queue, setQueue] = useState<QueuedChange[]>([])
  const [documents, setDocuments] = useState<CachedDocument[]>([])
  const [lastSync, setLastSync] = useState<string>()
  const [budget, setBudget] = useState<Record<string, unknown>>()
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const [loaded, setLoaded] = useState(false)
  const [online, setOnline] = useState(false)
  const [tab, setTab] = useState<'today' | 'money' | 'documents' | 'device'>('today')
  const [date, setDate] = useState(today())
  const currency = String(snapshot.settings.currency ?? 'USD')

  async function reload() {
    const [identity, snapshot, queue, documents, lastSync, budget] = await Promise.all([
      readState<CompanionIdentity>('identity'), localSnapshot(), queueChanges(), cachedDocuments(), readState<string>('lastSync'), readState<Record<string, unknown>>('budget'),
    ])
    setIdentity(identity); setSnapshot(snapshot); setQueue(queue); setDocuments(documents); setLastSync(lastSync); setBudget(budget); setLoaded(true)
  }
  async function synchronize() {
    setBusy(true)
    try {
      const result = await syncNow()
      const budget = await companionRpc<Record<string, unknown>>('budget.summary', { month: today().slice(0, 7), currency: String(result.snapshot.settings.currency ?? 'USD') })
      await saveState('budget', budget)
      await reload(); setOnline(true)
      setMessage(result.pending ? `${result.pending} change${result.pending === 1 ? '' : 's'} need attention in Device.` : 'Saved data is up to date.')
    } catch (error) { setOnline(false); setMessage(error instanceof Error ? error.message : 'Could not synchronize.') }
    finally { setBusy(false) }
  }
  useEffect(() => {
    reload().then(async () => { if (await readState('identity')) void synchronize() }).catch(error => { setMessage(String(error)); setLoaded(true) })
    if ('serviceWorker' in navigator && window.isSecureContext) navigator.serviceWorker.register('/sw.js', { scope: '/' }).catch(() => setMessage('Offline app installation failed. Reconnect and reopen Atlas before relying on offline access.'))
    let manifest = document.querySelector<HTMLLinkElement>('link[rel="manifest"]')
    if (!manifest) { manifest = document.createElement('link'); manifest.rel = 'manifest'; manifest.href = '/manifest.webmanifest'; document.head.appendChild(manifest) }
    if (!document.querySelector('link[rel="apple-touch-icon"]')) {
      const icon = document.createElement('link'); icon.rel = 'apple-touch-icon'; icon.href = '/apple-touch-icon.png'; document.head.appendChild(icon)
    }
    for (const [name, content] of [['apple-mobile-web-app-capable', 'yes'], ['apple-mobile-web-app-status-bar-style', 'black-translucent'], ['apple-mobile-web-app-title', 'Atlas']]) {
      if (!document.querySelector(`meta[name="${name}"]`)) { const meta = document.createElement('meta'); meta.name = name; meta.content = content; document.head.appendChild(meta) }
    }
    const onOnline = () => { if (document.visibilityState === 'visible') void readState('identity').then(identity => { if (identity) void synchronize() }) }
    const onOffline = () => setOnline(false)
    window.addEventListener('online', onOnline); window.addEventListener('offline', onOffline)
    return () => { window.removeEventListener('online', onOnline); window.removeEventListener('offline', onOffline) }
    // The initial startup and browser connectivity events deliberately own synchronization.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
  async function action(task: () => Promise<unknown>, success?: string) {
    setBusy(true)
    try { await task(); await reload(); if (success) setMessage(success) }
    catch (error) { setMessage(error instanceof Error ? error.message : 'The action could not be completed.') }
    finally { setBusy(false) }
  }

  const stays = snapshot.records.filter(r => r.kind === 'stay' && text(r, 'status') !== 'cancelled' && text(r, 'checkIn') <= date && text(r, 'checkOut') > date)
  const objectives = snapshot.records.filter(r => r.kind === 'objective' && (text(r, 'date') === date || (!r.date && stays.some(stay => stay.id === r.stayId))))
  const transport = snapshot.records.filter(r => {
    if (r.kind !== 'transport' || text(r, 'status') === 'cancelled') return false
    const departure = instantDate(text(r, 'departure'), text(r, 'departureTimezone'))
    const arrival = instantDate(text(r, 'arrival'), text(r, 'arrivalTimezone'))
    return departure === date || arrival === date || (departure < date && arrival > date)
  })
  const selectableDocuments = snapshot.records.filter(r => r.kind === 'document' && r.companionAvailable === true)

  if (!loaded) return <main className="companion"><div className="cp-empty">Opening your saved Atlas…</div></main>
  if (!identity) return <main className="companion"><div className="cp-pair">
    <div className="cp-brand">BIRTHRIGHT <span>ATLAS</span></div>
    <div className="cp-eyebrow">YOUR JOURNEY, WITH YOU</div><h1>A little closer<br />to everywhere.</h1>
    <p>Pair with your desktop to carry your agenda, chosen documents, and daily spending offline.</p>
    <form onSubmit={event => { event.preventDefault(); const data = new FormData(event.currentTarget); void action(async () => { await pairDevice(String(data.get('code')), String(data.get('name'))); await requestPersistentStorage(); await synchronize() }) }}>
      <label>This device<input name="name" required maxLength={80} defaultValue="My iPhone" autoComplete="off" /></label>
      <label>Desktop pairing code<input name="code" required autoComplete="off" autoCapitalize="none" spellCheck={false} placeholder="One-time code from Atlas host" /></label>
      <button className="cp-primary" disabled={busy} type="submit">{busy ? 'Pairing…' : 'Pair this device'} <span>↗</span></button>
    </form>
    <p className="cp-fine">On iPhone, open this trusted HTTPS address in Safari, then Share → Add to Home Screen. Pairing codes expire after 10 minutes. The desktop must be running for new synchronization.</p>
    {message && <p className="cp-message" role="status">{message}</p>}
  </div></main>

  return <main className="companion">
    <header className="cp-header"><div className="cp-brand">BIRTHRIGHT <span>ATLAS</span></div><button className="cp-sync" disabled={busy} onClick={() => void synchronize()} aria-label="Synchronize with desktop">{busy ? '◌' : '↻'}<span>{busy ? 'Syncing' : 'Sync'}</span></button></header>
    <div className="cp-status"><span className={online ? 'cp-dot online' : 'cp-dot'} />{online ? 'Desktop connected' : 'Saved on this phone'}{queue.length > 0 && <span className="cp-pill">{queue.length} pending</span>}</div>
    {message && <div className="cp-message" role="status">{message}<button onClick={() => setMessage('')} aria-label="Dismiss status">×</button></div>}

    {tab === 'today' && <section className="cp-content">
      <div className="cp-eyebrow">ROOM TO WANDER</div><h1>Your day,<br /><em>at your pace.</em></h1>
      <div className="cp-date-row"><span>{new Date(`${date}T12:00:00`).toLocaleDateString(undefined, { weekday: 'long', month: 'long', day: 'numeric' })}</span><input type="date" aria-label="Agenda date" value={date} onChange={e => setDate(e.target.value)} /></div>
      <div className="cp-section-title"><h2>Stay</h2><span>{stays.length ? 'YOUR HOME BASE' : 'OPEN HORIZONS'}</span></div>
      {stays.length ? stays.map(stay => <article className="cp-stay" key={stay.id}><div className="cp-eyebrow">{text(stay, 'context') === 'on_road' ? 'ON ROAD' : 'ON FOOT'} · {text(stay, 'type') || 'STAY'}</div><h2>{text(stay, 'name')}</h2><p>{text(stay, 'address') || 'Address not supplied'}</p><div className="cp-stay-dates"><span>ARRIVE <strong>{text(stay, 'checkIn')}</strong></span><span>DEPART <strong>{text(stay, 'checkOut')}</strong></span></div><div className="cp-actions">{Boolean(stay.address) && <a href={`https://maps.apple.com/?q=${encodeURIComponent(text(stay, 'address'))}`} target="_blank" rel="noreferrer">Directions ↗</a>}{/^https?:\/\//.test(text(stay, 'hostUrl')) && <a href={text(stay, 'hostUrl')} target="_blank" rel="noreferrer">Host conversation ↗</a>}</div>{Boolean(stay.contact) && <p className="cp-fine">Contact: {text(stay, 'contact')}</p>}</article>) : <div className="cp-empty">No stay saved for this night.{transport.some(r => r.overnight) ? ' Overnight travel is planned.' : ' Your next home base is still open.'}</div>}
      <div className="cp-section-title"><h2>In the day</h2><span>{objectives.length} OBJECTIVES</span></div>
      {objectives.map(objective => <article className="cp-objective" key={objective.id}><button aria-label={`${objective.status === 'done' ? 'Reopen' : 'Complete'} ${text(objective, 'name')}`} className={`cp-check ${objective.status === 'done' ? 'done' : ''}`} disabled={Boolean(objective._pending)} onClick={() => void action(() => queueRecord({ ...objective, status: objective.status === 'done' ? 'planned' : 'done' }), 'Objective saved on this phone. Sync to update the desktop.')}>{objective.status === 'done' ? '✓' : ''}</button><div><strong>{text(objective, 'name')}</strong><p>{text(objective, 'startTime') || 'When you find a moment'} · {text(objective, 'category') || 'Objective'}{objective._pending ? ' · pending sync' : ''}</p></div></article>)}
      {!objectives.length && <p className="cp-fine cp-spaced">A little breathing room. No objectives saved for today.</p>}
      {transport.length > 0 && <><div className="cp-section-title"><h2>On the way</h2><span>TRANSPORT</span></div>{transport.map(leg => <article className="cp-transport" key={leg.id}><span aria-hidden="true">↗</span><div><strong>{text(leg, 'origin')} → {text(leg, 'destination')}</strong><p>{text(leg, 'mode')} · {instantLabel(text(leg, 'departure'), text(leg, 'departureTimezone'))}<br />Arrive {instantLabel(text(leg, 'arrival'), text(leg, 'arrivalTimezone'))}</p></div></article>)}</>}
      <button className="cp-secondary cp-wide" onClick={() => void action(async () => { const result = await companionRpc<{ filename: string; content: string }>('calendar.export'); download(new Blob([result.content], { type: 'text/calendar' }), result.filename) })}>Export saved calendar (.ics)</button>
      <p className="cp-fine">Calendar export needs a reachable desktop. Add it to your calendar for the notification capabilities supported by that calendar app.</p>
    </section>}

    {tab === 'money' && <section className="cp-content"><div className="cp-eyebrow">SMALL THINGS, ACCOUNTED FOR</div><h1>Room in<br /><em>your day.</em></h1>
      {budget ? <article className="cp-budget"><span>REMAINING FLEXIBLE ALLOWANCE</span><strong>{money(budget.dailyAllowanceMinor, String(budget.currency ?? currency))}<small> / eligible day</small></strong><p>{money(budget.allowanceRemainingMinor, String(budget.currency ?? currency))} across {String(budget.remainingDays ?? '—')} remaining days · {String(budget.month ?? '')}</p><div><span>Forecast</span><b>{money(budget.forecastMinor, String(budget.currency ?? currency))}</b></div><div><span>Calendar days</span><b>{String(budget.daysInMonth ?? '—')}</b></div><p className="cp-fine">Last synchronized calculation. Pending phone expenses are listed below and are included in the next successful sync.</p></article> : <p className="cp-empty">Sync once to bring your monthly budget along.</p>}
      <div className="cp-section-title"><h2>A small expense</h2><span>SAVE NOW, SYNC LATER</span></div>
      <form className="cp-expense-form" onSubmit={event => { event.preventDefault(); const form = event.currentTarget; const data = new FormData(form); void action(async () => { const currency = String(data.get('currency')).toUpperCase(); await queueRecord({ id: crypto.randomUUID(), kind: 'expense', name: String(data.get('name')), date: String(data.get('date')), currency, amountMinor: minorUnits(String(data.get('amount')), currency), category: String(data.get('category')) }); form.reset() }, 'Expense saved on this phone. Sync when the desktop is reachable.') }}>
        <label>What was it?<input name="name" placeholder="Groceries, a coffee, a small discovery" required maxLength={140} /></label><div className="cp-fields"><label>Amount<input name="amount" inputMode="decimal" placeholder="0.00" required /></label><label>Currency<input name="currency" defaultValue={currency} required pattern="[A-Za-z]{3}" maxLength={3} /></label></div>
        <div className="cp-fields"><label>Date<input type="date" name="date" defaultValue={today()} required /></label><label>Category<select name="category"><option value="groceries">Groceries</option><option value="food">Food & drink</option><option value="transport">Transport</option><option value="laundry">Laundry</option><option value="other">Other</option></select></label></div>
        <button type="submit" className="cp-primary" disabled={busy}>Save expense <span>+</span></button>
      </form>
      <div className="cp-section-title"><h2>Recent spending</h2></div>{snapshot.records.filter(r => r.kind === 'expense').sort((a, b) => text(b, 'date').localeCompare(text(a, 'date'))).slice(0, 12).map(expense => <article className="cp-expense" key={expense.id}><div><strong>{text(expense, 'name')}</strong><p>{text(expense, 'date')}{expense._pending ? ' · pending sync' : ''}</p></div><b>{money(expense.amountMinor, text(expense, 'currency') || currency)}</b></article>)}
    </section>}

    {tab === 'documents' && <section className="cp-content"><div className="cp-eyebrow">THE USEFUL THINGS</div><h1>Keep it<br /><em>close.</em></h1><p>Choose which documents stay on this phone. First mark a document available to the companion on your desktop.</p>
      <div className="cp-section-title"><h2>Offline documents</h2><span>{(documents.reduce((sum, d) => sum + d.blob.size, 0) / 1024 / 1024).toFixed(1)} / 50 MiB</span></div>
      {selectableDocuments.map(record => { const saved = documents.find(d => d.id === record.id); return <article className="cp-document" key={record.id}><div><strong>{text(record, 'filename') || text(record, 'name') || 'Document'}</strong><p>{saved ? `Saved ${new Date(saved.savedAt).toLocaleDateString()}` : 'Available from desktop'}</p></div><div className="cp-actions">{saved ? <><button onClick={() => download(saved.blob, saved.filename)}>Open</button><button onClick={() => void action(() => removeDocument(saved.id), 'Removed from phone storage.')}>Remove</button></> : <button disabled={busy} onClick={() => void action(() => keepDocument(record.id), 'Document is available offline on this phone.')}>Keep offline</button>}</div></article> })}
      {documents.filter(doc => !selectableDocuments.some(record => record.id === doc.id)).map(doc => <article className="cp-document" key={doc.id}><div><strong>{doc.filename}</strong><p>Previously saved copy</p></div><div className="cp-actions"><button onClick={() => download(doc.blob, doc.filename)}>Open</button><button onClick={() => void action(() => removeDocument(doc.id))}>Remove</button></div></article>)}
      {!selectableDocuments.length && !documents.length && <p className="cp-empty">No documents selected yet.</p>}
      <p className="cp-fine">Phone copies remain until you remove them, even if sharing is later disabled on the desktop. Browser storage can be cleared by the OS. Save critical tickets in a separate secure location too.</p>
    </section>}

    {tab === 'device' && <section className="cp-content"><div className="cp-eyebrow">YOUR PRIVATE CONNECTION</div><h1>One journey.<br /><em>Within reach.</em></h1><article className="cp-device-card"><h2>{identity.name}</h2><p>Paired {new Date(identity.pairedAt).toLocaleDateString()}</p><p>Last sync: {lastSync ? new Date(lastSync).toLocaleString() : 'Not synchronized yet'}</p><button className="cp-secondary" disabled={busy} onClick={() => void synchronize()}>Synchronize now</button></article>
      <div className="cp-section-title"><h2>Waiting to sync</h2><span>{queue.length} CHANGES</span></div>
      {queue.map(change => <article className="cp-queue-item" key={change.id}><strong>{String(change.payload.record.name ?? 'Saved change')}</strong><p>{change.state === 'pending' ? 'Saved on this phone, awaiting the desktop.' : change.error}</p>{change.state === 'conflict' && <p className="cp-fine">The desktop record changed after this phone last synchronized. Review both versions before applying a choice.</p>}<details><summary>Review local change</summary><pre>{JSON.stringify(change.payload.record, null, 2)}</pre></details>{change.state === 'conflict' && <details><summary>Review desktop version</summary><pre>{JSON.stringify(snapshot.records.find(r => r.id === change.payload.record.id) ?? { status: 'Not present on desktop' }, null, 2)}</pre></details>}<div className="cp-actions"><button onClick={() => void action(() => removeChange(change.id), 'Local change discarded; desktop version kept.')}>Discard local change</button>{change.state === 'conflict' && <button onClick={() => void action(async () => { const latest = await readState<Snapshot>('snapshot'); const current = latest?.records.find(r => r.id === change.payload.record.id); if (!current?.version) throw new Error('The desktop record no longer exists. Save a new expense instead.'); await removeChange(change.id); await saveChange({ ...change, id: crypto.randomUUID(), state: 'pending', error: undefined, payload: { ...change.payload, expectedVersion: current.version } }) }, 'Your local version is queued against the latest desktop version.')}>Apply my change</button>}</div></article>)}
      {!queue.length && <p className="cp-empty">Nothing waiting. A little less to carry.</p>}
      <h2 className="cp-section-heading">Offline & privacy</h2><p className="cp-fine">Previously synchronized records work while the desktop is off. New sync, email imports, and scans require a running reachable desktop. Atlas does not promise notifications from a closed iPhone PWA. Export calendar events to use your calendar app’s delivery features.</p><p className="cp-fine">This phone stores its paired credential, saved records, and chosen documents in browser storage. Atlas does not add application-level encryption to that storage. Use your device passcode and revoke lost devices from the desktop. Revocation prevents future access; it cannot erase an offline copy.</p>
      <button className="cp-secondary cp-wide" onClick={() => void action(async () => { const granted = await requestPersistentStorage(); setMessage(granted ? 'Persistent browser storage granted.' : 'The browser did not grant persistent storage. Keep an independent copy of essential documents.') })}>Request persistent storage</button>
      <button className="cp-danger cp-wide" onClick={() => { if (window.confirm(`Remove this device’s credential, records, documents, and ${queue.length} unsynchronized changes? This does not revoke the device on the desktop.`)) void action(async () => { await clearPhoneData(); setMessage('Phone data cleared. Revoke this device on the desktop if it is no longer in use.') }) }}>Clear all phone data</button>
    </section>}

    <nav className="cp-nav" aria-label="Companion navigation">{(['today', 'money', 'documents', 'device'] as const).map((name, index) => <button key={name} className={tab === name ? 'active' : ''} onClick={() => setTab(name)} aria-current={tab === name ? 'page' : undefined}><span aria-hidden="true">{['◫', '◉', '▱', '⌘'][index]}</span>{name === 'money' ? 'Budget' : name[0].toUpperCase() + name.slice(1)}</button>)}</nav>
  </main>
}
