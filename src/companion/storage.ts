export type AtlasRecord = Record<string, unknown> & { id: string; kind: string; version?: number }
export type Snapshot = { records: AtlasRecord[]; settings: Record<string, unknown> }
export type QueuedChange = {
  id: string; method: 'records.save'; payload: { record: AtlasRecord; expectedVersion: number }
  createdAt: string; state: 'pending' | 'conflict' | 'error'; error?: string
}
export type CompanionIdentity = { deviceId: string; name: string; token: string; pairedAt: string }
export type CachedDocument = { id: string; filename: string; mimeType: string; blob: Blob; savedAt: string }

const DB_NAME = 'birthright-atlas-companion'
let connection: Promise<IDBDatabase> | undefined

function database(): Promise<IDBDatabase> {
  if (!connection) connection = new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1)
    request.onupgradeneeded = () => {
      const db = request.result
      db.createObjectStore('state')
      db.createObjectStore('queue', { keyPath: 'id' })
      db.createObjectStore('documents', { keyPath: 'id' })
    }
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => { connection = undefined; reject(new Error('Phone storage is unavailable. Allow website storage to use Atlas offline.')) }
    request.onblocked = () => reject(new Error('Close other Atlas tabs to upgrade phone storage.'))
  })
  return connection
}

async function transaction<T>(storeName: string, mode: IDBTransactionMode, perform: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await database()
  return new Promise((resolve, reject) => {
    const tx = db.transaction(storeName, mode)
    const request = perform(tx.objectStore(storeName))
    let result: T
    request.onsuccess = () => { result = request.result }
    tx.oncomplete = () => resolve(result)
    tx.onerror = () => reject(tx.error ?? new Error('Atlas could not save phone data. Check available storage.'))
    tx.onabort = () => reject(tx.error ?? new Error('Atlas phone storage change was interrupted.'))
  })
}

export const readState = <T>(key: string) => transaction<T | undefined>('state', 'readonly', store => store.get(key))
export const saveState = (key: string, value: unknown) => transaction('state', 'readwrite', store => store.put(value, key))
export const queueChanges = () => transaction<QueuedChange[]>('queue', 'readonly', store => store.getAll())
export const saveChange = (change: QueuedChange) => transaction('queue', 'readwrite', store => store.put(change))
export const removeChange = (id: string) => transaction('queue', 'readwrite', store => store.delete(id))
export const cachedDocuments = () => transaction<CachedDocument[]>('documents', 'readonly', store => store.getAll())
export const saveDocument = (document: CachedDocument) => transaction('documents', 'readwrite', store => store.put(document))
export const removeDocument = (id: string) => transaction('documents', 'readwrite', store => store.delete(id))

export async function clearPhoneData(): Promise<void> {
  const db = await database()
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(['state', 'queue', 'documents'], 'readwrite')
    for (const name of ['state', 'queue', 'documents']) tx.objectStore(name).clear()
    tx.oncomplete = () => resolve()
    tx.onerror = () => reject(tx.error)
  })
}

export async function localSnapshot(): Promise<Snapshot> {
  const snapshot = await readState<Snapshot>('snapshot') ?? { records: [], settings: {} }
  const pending = await queueChanges()
  const records = new Map(snapshot.records.map(record => [record.id, record]))
  for (const item of pending) {
    if (item.state !== 'conflict') records.set(item.payload.record.id, { ...item.payload.record, _pending: true })
  }
  return { ...snapshot, records: [...records.values()] }
}
