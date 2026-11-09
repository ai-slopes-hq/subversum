import { test } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync } from 'node:fs';

const src = readFileSync(new URL('./sw.js', import.meta.url), 'utf8');

function load() {
  const listeners = {};
  const opened = [];
  const cache = { addAll: async () => {}, put: async () => {} };
  const caches = {
    open: async name => { opened.push(name); return cache; },
    match: async () => undefined,
    keys: async () => [],
    delete: async () => true,
  };
  const self = { addEventListener: (t, f) => { listeners[t] = f; }, skipWaiting: async () => {}, clients: { claim: async () => {} } };
  const fetched = [];
  const ctx = {
    self, caches, URL, location: { origin: 'https://x.test' }, Promise,
    fetch: async req => { fetched.push(req.url); return { ok: false, clone() { return this; } }; },
  };
  vm.runInNewContext(src, ctx);
  return { listeners, opened, fetched };
}

function fetchEvent(listeners, path, { method = 'GET', mode = 'cors', origin = 'https://x.test' } = {}) {
  let responded = false;
  listeners.fetch({
    request: { method, mode, url: origin + path },
    respondWith: () => { responded = true; },
  });
  return responded;
}

test('next_navigation_is_not_intercepted', () => {
  const { listeners } = load();
  assert.equal(fetchEvent(listeners, '/next/', { mode: 'navigate' }), false);
  assert.equal(fetchEvent(listeners, '/next/index.html', { mode: 'navigate' }), false);
});

test('next_assets_are_not_intercepted', () => {
  const { listeners } = load();
  assert.equal(fetchEvent(listeners, '/next/assets/a.js'), false);
  assert.equal(fetchEvent(listeners, '/next/engine.wasm'), false);
});

test('next_prefix_does_not_swallow_lookalikes', () => {
  const { listeners } = load();
  assert.equal(fetchEvent(listeners, '/nextgen/a.js'), true);
});

test('root_navigation_still_handled', () => {
  const { listeners } = load();
  assert.equal(fetchEvent(listeners, '/', { mode: 'navigate' }), true);
});

test('legacy_assets_still_handled', () => {
  const { listeners } = load();
  assert.equal(fetchEvent(listeners, '/icons/icon-192.png'), true);
});

test('vercel_still_skipped', () => {
  const { listeners } = load();
  assert.equal(fetchEvent(listeners, '/_vercel/insights/script.js'), false);
});

test('cache_name_is_v3', async () => {
  const { listeners, opened } = load();
  let p;
  listeners.install({ waitUntil: x => { p = x; } });
  await p;
  assert.deepEqual(opened, ['subversum-v3']);
});
