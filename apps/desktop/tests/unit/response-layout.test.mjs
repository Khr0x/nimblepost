import assert from 'node:assert/strict';
import { test } from 'node:test';
import { RESPONSE_DEFAULT_HEIGHT, responseMinHeight, responseMaxHeight, clampResponseHeight, readResponseHeight, saveResponseHeight } from '../../src/lib/features/responses/response-layout.ts';

test('Response limita su altura, reserva espacio para la petición y recupera la preferencia', (t) => {
  const originalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
  t.after(() => {
    if (originalStorage) Object.defineProperty(globalThis, 'localStorage', originalStorage);
    else delete globalThis.localStorage;
  });
  let saved = null;
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
    getItem: () => saved,
    setItem: (key, value) => { assert.equal(key, 'nimblepost.responseHeight'); saved = value; },
  } });
  assert.equal(readResponseHeight(), RESPONSE_DEFAULT_HEIGHT);
  assert.equal(clampResponseHeight(-100, 700), 180);
  assert.equal(responseMaxHeight(700), 490);
  assert.equal(clampResponseHeight(10000, 700), responseMaxHeight(700));
  assert.equal(responseMaxHeight(477), 277);
  assert.equal(clampResponseHeight(320, 477), 277);
  assert.equal(responseMinHeight(250), 125);
  assert.equal(clampResponseHeight(320, 250), 125);
  saveResponseHeight(410);
  assert.equal(readResponseHeight(), 410);
  assert.equal(clampResponseHeight(readResponseHeight(), 477), 277);
  assert.equal(readResponseHeight(), 410);
  saved = 'invalid';
  assert.equal(readResponseHeight(), RESPONSE_DEFAULT_HEIGHT);
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, get() { throw new Error('Storage blocked'); } });
  assert.equal(readResponseHeight(), RESPONSE_DEFAULT_HEIGHT);
  t.mock.method(console, 'warn', () => {});
  assert.doesNotThrow(() => saveResponseHeight(320));
});
