import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readTheme, applyTheme, saveTheme } from '../../src/lib/layout/theme.ts';

test('dark por defecto, preferencia persistida y fallback cuando storage no está disponible', (t) => {
  const originalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
  const originalDocument = Object.getOwnPropertyDescriptor(globalThis, 'document');
  t.after(() => {
    for (const [name, descriptor] of [['localStorage', originalStorage], ['document', originalDocument]]) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else delete globalThis[name];
    }
  });
  const values = new Map();
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  } });
  Object.defineProperty(globalThis, 'document', { configurable: true, value: { documentElement: { dataset: {} } } });
  assert.equal(readTheme(), 'dark');
  applyTheme(readTheme());
  assert.equal(document.documentElement.dataset.theme, 'dark');
  saveTheme('light');
  assert.equal(readTheme(), 'light');
  assert.equal(document.documentElement.dataset.theme, 'light');
  saveTheme('dark');
  assert.equal(readTheme(), 'dark');
  values.set('nimblepost.theme', 'invalid');
  assert.equal(readTheme(), 'dark');
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, get() { throw new Error('Storage blocked'); } });
  assert.equal(readTheme(), 'dark');
  t.mock.method(console, 'warn', () => {});
  saveTheme('light');
  assert.equal(document.documentElement.dataset.theme, 'light');
});
