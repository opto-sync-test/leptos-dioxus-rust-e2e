const DATABASE = "opto-sync-rust";
const STORE = "mutations";
const SYNC_TAG = "opto-sync-multiplex";

function openDatabase() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE, 1);
    request.onupgradeneeded = () => {
      const store = request.result.createObjectStore(STORE, { keyPath: "id" });
      store.createIndex("createdAt", "createdAt");
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function transact(mode, operation) {
  const database = await openDatabase();
  return new Promise((resolve, reject) => {
    const transaction = database.transaction(STORE, mode);
    const result = operation(transaction.objectStore(STORE));
    transaction.oncomplete = () => {
      database.close();
      resolve(result instanceof IDBRequest ? result.result : result);
    };
    transaction.onerror = () => reject(transaction.error);
    transaction.onabort = () => reject(transaction.error);
  });
}

async function enqueue(mutation) {
  const durable = Object.freeze({
    ...mutation,
    id: mutation.id ?? crypto.randomUUID(),
    createdAt: mutation.createdAt ?? Date.now(),
  });
  await transact("readwrite", (store) => store.put(durable));
  const registration = await self.registration;
  await registration.sync?.register(SYNC_TAG);
}

async function pendingSnapshot() {
  const rows = await transact("readonly", (store) => store.getAll());
  return Object.freeze(
    rows
      .sort((left, right) => left.createdAt - right.createdAt)
      .map((row) => Object.freeze({ ...row })),
  );
}

async function sendLane(lane, snapshot) {
  const response = await fetch(`/api/sync/${lane}`, {
    method: "POST",
    headers: { "content-type": "application/json", "x-opto-sync-lane": lane },
    body: JSON.stringify(snapshot),
  });
  if (!response.ok) throw new Error(`${lane} lane returned ${response.status}`);
  return response.json();
}

async function flush() {
  const snapshot = await pendingSnapshot();
  if (snapshot.length === 0) return;

  const outcomes = await Promise.allSettled([
    sendLane("upload", snapshot),
    sendLane("realtime", snapshot),
  ]);
  if (outcomes.some((outcome) => outcome.status === "rejected")) {
    throw new Error("multiplex flush incomplete; durable batch retained for retry");
  }

  await transact("readwrite", (store) => {
    for (const mutation of snapshot) store.delete(mutation.id);
  });
  const windows = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
  for (const window of windows) window.postMessage({ type: "OPTO_SYNC_FLUSHED", outcomes });
}

self.addEventListener("install", (event) => event.waitUntil(self.skipWaiting()));
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));
self.addEventListener("message", (event) => {
  if (event.data?.type === "OPTO_SYNC_ENQUEUE") event.waitUntil(enqueue(event.data.mutation));
  if (event.data?.type === "OPTO_SYNC_WAKE") event.waitUntil(flush());
});
self.addEventListener("sync", (event) => {
  if (event.tag === SYNC_TAG) event.waitUntil(flush());
});
self.addEventListener("periodicsync", (event) => {
  if (event.tag === SYNC_TAG) event.waitUntil(flush());
});
self.addEventListener("online", () => flush());
