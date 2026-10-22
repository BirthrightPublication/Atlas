/* API responses and documents are never cached implicitly. Phone data is kept
   in explicit IndexedDB stores managed by the companion's storage controls. */
const CACHE = 'atlas-shell-v2';
const SHELL = ['/companion', '/manifest.webmanifest', '/companion-icon.svg'];
self.addEventListener('install', event => {
  event.waitUntil((async () => {
    const cache = await caches.open(CACHE);
    await cache.addAll(SHELL);
    const html = await (await cache.match('/companion')).text();
    const assets = [...html.matchAll(/(?:src|href)=["']([^"']+)["']/g)]
      .map(match => new URL(match[1], self.location.origin))
      .filter(url => url.origin === self.location.origin && /^\/assets\/[^/]+\.(js|css|woff2?)$/.test(url.pathname))
      .map(url => url.href);
    await cache.addAll([...new Set(assets)]);
  })());
});
self.addEventListener('activate', event => {
  event.waitUntil(caches.keys().then(keys => Promise.all(keys.filter(key => key.startsWith('atlas-shell-') && key !== CACHE).map(key => caches.delete(key)))).then(() => self.clients.claim()));
});
self.addEventListener('fetch', event => {
  const request = event.request;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin || request.method !== 'GET' || url.pathname.startsWith('/api/')) return;
  if (request.mode === 'navigate' && url.pathname.startsWith('/companion')) {
    event.respondWith(fetch(request).then(response => {
      if (response.ok) { const copy = response.clone(); event.waitUntil(caches.open(CACHE).then(cache => cache.put('/companion', copy))); }
      return response;
    }).catch(() => caches.match('/companion')));
    return;
  }
  if (/^\/assets\/[^/]+\.(js|css|woff2?)$/.test(url.pathname) || SHELL.includes(url.pathname)) {
    event.respondWith(caches.match(request).then(cached => cached || fetch(request).then(response => {
      if (response.ok) { const copy = response.clone(); event.waitUntil(caches.open(CACHE).then(cache => cache.put(request, copy))); }
      return response;
    })));
  }
});
