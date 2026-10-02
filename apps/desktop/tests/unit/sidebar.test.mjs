import assert from 'node:assert/strict';
import { test } from 'node:test';
import { SIDEBAR_MIN_WIDTH, sidebarMaxWidth, clampSidebarWidth, readSidebarWidth, saveSidebarWidth } from '../../src/lib/layout/sidebar.ts';

test('el ancho del sidebar respeta límites, reserva espacio al editor y recupera la preferencia', (t) => {
  const originalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
  t.after(() => {
    if (originalStorage) Object.defineProperty(globalThis, 'localStorage', originalStorage);
    else delete globalThis.localStorage;
  });
  let saved = null;
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
    getItem: () => saved,
    setItem: (key, value) => { assert.equal(key, 'nimblepost.sidebarWidth'); saved = value; },
  } });
  assert.equal(readSidebarWidth(), SIDEBAR_MIN_WIDTH);
  assert.equal(clampSidebarWidth(-100), 245);
  assert.equal(clampSidebarWidth(NaN), 245);
  assert.equal(clampSidebarWidth(10000), 480);
  assert.equal(sidebarMaxWidth(800), 320);
  assert.equal(clampSidebarWidth(480, 800), 320);
  assert.equal(clampSidebarWidth(480, 600), 245);
  saveSidebarWidth(370);
  assert.equal(readSidebarWidth(), 370);
  assert.equal(clampSidebarWidth(readSidebarWidth(), 800), 320);
  assert.equal(readSidebarWidth(), 370);
  saved = 'invalid';
  assert.equal(readSidebarWidth(), 245);
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, get() { throw new Error('Storage blocked'); } });
  assert.equal(readSidebarWidth(), 245);
  t.mock.method(console, 'warn', () => {});
  assert.doesNotThrow(() => saveSidebarWidth(320));
});
