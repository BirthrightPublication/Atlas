import {
  cachedDocuments, localSnapshot, queueChanges, readState, removeChange, saveChange,
  saveDocument, saveState, type AtlasRecord, type CompanionIdentity, type Snapshot,
} from './storage'

export class CompanionError extends Error {
  constructor(message: string, public status: number) { super(message) }
}

async function request<T>(path: string, body?: unknown, token?: string): Promise<T> {
  const controller = new AbortController()
  const timer = window.setTimeout(() => controller.abort(), 20_000)
  try {
    const response = await fetch(path, {
      method: body === undefined ? 'GET' : 'POST',
      headers: { ...(body === undefined ? {} : { 'Content-Type': 'application/json' }), ...(token ? { Authorization: `Bearer ${token}` } : {}) },
      body: body === undefined ? undefined : JSON.stringify(body),
      credentials: 'omit', cache: 'no-store', redirect: 'error', signal: controller.signal,
    })
    const data = await response.json()
    if (!response.ok) throw new CompanionError(typeof data.error === 'string' ? data.error : `Atlas returned ${response.status}`, response.status)
    return data as T
  } catch (error) {
    if (error instanceof CompanionError) throw error
    throw new CompanionError('The desktop is unreachable. Saved data remains available; new changes wait on this phone.', 0)
  } finally { window.clearTimeout(timer) }
}

export async function pairDevice(code: string, name: string): Promise<CompanionIdentity> {
  if (!window.isSecureContext) throw new Error('Pairing requires trusted HTTPS. Open the private HTTPS address configured on your desktop.')
  const identity = await request<CompanionIdentity>('/api/pair', { code: code.trim(), name: name.trim() })
  await saveState('identity', identity)
  return identity
}

export async function companionRpc<T>(method: string, payload: unknown = {}, requestId?: string): Promise<T> {
  const identity = await readState<CompanionIdentity>('identity')
  if (!identity) throw new CompanionError('Pair this phone first.', 401)
  return request<T>('/api/rpc', { method, payload, requestId }, identity.token)
}

let syncing: Promise<{ snapshot: Snapshot; lastSync: string; pending: number }> | undefined
export function syncNow() {
  if (!syncing) syncing = performSync().finally(() => { syncing = undefined })
  return syncing
}

async function performSync() {
  // Read-only handshake verifies credentials before touching the offline queue.
  await companionRpc<Snapshot>('snapshot')
  const queue = (await queueChanges()).sort((a, b) => a.createdAt.localeCompare(b.createdAt))
  for (const change of queue) {
    if (change.state === 'conflict') continue
    try {
      await companionRpc(change.method, change.payload, change.id)
      await removeChange(change.id)
    } catch (error) {
      if (error instanceof CompanionError && (error.status === 0 || error.status === 401)) throw error
      await saveChange({ ...change, state: error instanceof CompanionError && error.status === 409 ? 'conflict' : 'error', error: error instanceof Error ? error.message : 'Could not synchronize change' })
    }
  }
  const snapshot = await companionRpc<Snapshot>('snapshot')
  const lastSync = new Date().toISOString()
  await saveState('snapshot', snapshot)
  await saveState('lastSync', lastSync)
  return { snapshot: await localSnapshot(), lastSync, pending: (await queueChanges()).length }
}

export async function queueRecord(record: AtlasRecord): Promise<void> {
  const existing = (await queueChanges()).find(item => item.payload.record.id === record.id)
  if (existing) throw new Error('Synchronize or resolve the pending change for this record before editing it again.')
  await saveChange({ id: crypto.randomUUID(), method: 'records.save', payload: { record, expectedVersion: record.version ?? 0 }, createdAt: new Date().toISOString(), state: 'pending' })
}

export async function keepDocument(id: string): Promise<void> {
  const result = await companionRpc<{ filename: string; contentBase64: string; mimeType: string }>('documents.read', { id })
  if (result.contentBase64.length > 8 * 1024 * 1024 * 4 / 3 + 4) throw new Error('Documents are limited to 8 MiB each.')
  const bytes = Uint8Array.from(atob(result.contentBase64), c => c.charCodeAt(0))
  const existing = await cachedDocuments()
  if (existing.filter(d => d.id !== id).reduce((sum, d) => sum + d.blob.size, bytes.byteLength) > 50 * 1024 * 1024) throw new Error('Atlas has a 50 MiB offline document limit. Remove a saved document first.')
  const mimeType = ['application/pdf', 'image/png', 'image/jpeg', 'text/plain', 'text/calendar'].includes(result.mimeType) ? result.mimeType : 'application/octet-stream'
  await saveDocument({ id, filename: result.filename, mimeType, blob: new Blob([bytes], { type: mimeType }), savedAt: new Date().toISOString() })
}

export async function requestPersistentStorage(): Promise<boolean> {
  return navigator.storage?.persist ? navigator.storage.persist() : false
}
