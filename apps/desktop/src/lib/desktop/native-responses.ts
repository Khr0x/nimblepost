// Loaded only by the validation build. Actions use the same handlers as the UI.
import { emit } from '@tauri-apps/api/event';
import { tick } from 'svelte';
import type { MemoryControls } from './native-validation';
import { stopProfiling } from './native-profile';

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const frames = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
const ensure = (ok: unknown, message: string) => { if (!ok) throw new Error(message); };
const digest = async (text: string) => [...new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text)))].map(byte => byte.toString(16).padStart(2, '0')).join('');

export async function validateResponses(controls: MemoryControls) {
  stopProfiling();
  const send = (url: string, raw = false) => {
    controls.provideSecret('np013-token', 'NP013_RUNTIME_SECRET_CANARY');
    return raw ? controls.sendRaw(url) : controls.send(url);
  };
  let index = 0;
  let actions: { name: string; durationMs: number }[] = [];
  const wait = async (condition: () => boolean, message: string) => {
    const deadline = performance.now() + 15000;
    while (!condition()) { ensure(performance.now() < deadline, message); await delay(10); }
  };
  const clean = () => {
    const state = controls.inspect();
    ensure(!state.error && !state.recoveryWarning, state.error || state.recoveryWarning);
    return state;
  };
  const action = async (name: string, perform: () => Promise<void> | void, visible: () => boolean) => {
    const started = performance.now();
    await perform(); await tick(); await frames();
    ensure(visible(), `Action did not become visible: ${name}`);
    actions.push({ name, durationMs: performance.now() - started });
  };
  const phase = async (cycle: number, name: string, extra: Record<string, unknown> = {}, settle = true) => {
    await tick(); await frames();
    const state = controls.inspect();
    ensure(!state.recoveryWarning, state.recoveryWarning);
    await emit('native-validation-responses', { index: index++, cycle, name, ...state, actions, ...extra });
    actions = [];
    // Three seconds of settling, then six host samples 200 ms apart.
    if (settle) await delay(4500);
  };
  const complete = async () => {
    while (controls.inspect().activeOffset < controls.inspect().bodyBytes) {
      const previous = controls.inspect().activeOffset;
      await action('chunk', controls.more, () => !controls.inspect().chunkLoading && controls.inspect().activeOffset > previous);
      clean();
    }
    ensure(controls.inspect().activeOffset === controls.inspect().bodyBytes, 'Incomplete response');
  };
  const content = async () => ({ sha256: await digest(controls.body()), bytes: new TextEncoder().encode(controls.body()).byteLength });
  const copy = async (cycle: number, kind: string) => {
    const button = document.querySelector<HTMLButtonElement>('[aria-label="Copy response body"]');
    ensure(button && !button.disabled, 'Complete response cannot be copied');
    const rect = button!.getBoundingClientRect();
    // The Linux host injects a real X11 click and independently reads the clipboard.
    await phase(cycle, 'copy', { kind, content: await content(), point: { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2, innerHeight: window.innerHeight } }, false);
    await wait(() => document.querySelector('.response-panel [role="status"]')?.textContent === 'Response copied.', 'Real clipboard action did not finish');
    await delay(1000);
  };
  const inspectView = () => {
    const viewport = document.querySelector<HTMLElement>('.response-text-viewport');
    ensure(viewport && viewport.clientHeight > 0, 'Large response viewport is missing');
    const rows = document.querySelectorAll('[data-response-line]');
    ensure(rows.length > 0 && rows.length <= Math.ceil(viewport!.clientHeight / 21.6) + 13, 'Response DOM is not bounded');
    return viewport!;
  };
  await phase(0, 'baseline');
  for (let cycle = 1; cycle <= 3; cycle++) {
    if (cycle > 1) await controls.reset(1);
    await controls.open('request-00000.yml');
    for (let request = 1; request < 64; request++) await controls.open(`request-${String(request).padStart(5, '0')}.yml`);
    ensure(clean().tabs.length === 64, 'Expected 64 tabs');
    const tabs = controls.inspect().tabs;
    const loaded = [];
    for (const tab of tabs.slice(-4)) {
      await controls.activate(tab); await send(`${controls.url}/text`); await complete();
      loaded.push(await content());
    }
    for (const [i, tab] of tabs.slice(-4).entries()) {
      await controls.activate(tab);
      ensure(await digest(controls.body()) === loaded[i].sha256, 'Cached response changed across tabs');
    }
    await controls.activate(tabs[0]); await controls.activate(tabs.at(-1)!);
    await phase(cycle, 'four-responses', { content: await content(), loaded });
    await send(`${controls.url}/json`); await complete(); clean();
    ensure(document.querySelector('.response-body-note')?.textContent?.includes('too large'), 'Large JSON must retain explicit formatting fallback');
    await phase(cycle, 'json', { content: await content(), renderedRows: document.querySelectorAll('[data-response-line]').length });
    await copy(cycle, 'json');
    await send(`${controls.url}/line`); await complete(); clean();
    const lineHash = await digest(controls.body());
    const viewport = inspectView();
    await action('scroll', () => { viewport.scrollTop = viewport.scrollHeight; viewport.dispatchEvent(new Event('scroll')); }, () => {
      const count = Number(viewport.dataset.responseLines);
      return !!document.querySelector(`[data-response-line="${count - 1}"]`);
    });
    const toggle = document.querySelector<HTMLButtonElement>('[aria-label="Wrap response lines"]');
    const wasWrapped = toggle?.getAttribute('aria-pressed');
    await action('wrap', () => toggle!.click(), () => toggle?.getAttribute('aria-pressed') !== wasWrapped);
    await action('tab-empty', () => controls.activate(tabs[0]), () => !document.querySelector('.response-panel') && controls.inspect().activeTab === tabs[0]);
    await action('tab-response', () => controls.activate(tabs.at(-1)!), () => !!document.querySelector('.response-panel') && controls.inspect().activeTab === tabs.at(-1));
    ensure(await digest(controls.body()) === lineHash, 'Tab switch changed the response');
    await phase(cycle, 'line', { content: await content() });
    await copy(cycle, 'line');
    await send(`${controls.url}/excess`, true);
    ensure(/16777216/.test(controls.inspect().error) && !controls.inspect().bodyBytes && !controls.inspect().activeBodyChars, 'Excess must be explicit and retain no partial body');
    await phase(cycle, 'excess');
    const pending = send(`${controls.url}/slow`, true);
    await wait(() => controls.inspect().busy, 'Request did not start');
    await delay(750);
    const cancel = Array.from(document.querySelectorAll<HTMLButtonElement>('button')).find(button => button.textContent?.trim() === 'Cancel');
    ensure(cancel && !cancel.disabled, 'Cancellation must be available during transfer');
    await action('cancel', async () => { cancel!.click(); await pending; }, () => !controls.inspect().busy && /cancelada/.test(controls.inspect().error));
    await send(`${controls.url}/text`); await complete(); clean();
    await phase(cycle, 'after-cancel', { content: await content() });
    for (const tab of controls.inspect().tabs) await controls.close(tab);
    const closed = clean();
    ensure(!closed.tabs.length && !closed.activeBodyChars && !closed.cachedBodyChars && !closed.bodyBytes, 'Frontend bodies survived closing all tabs');
    await phase(cycle, 'closed');
    await controls.reset();
    const reset = clean();
    ensure(!reset.labels && !reset.attempts && !reset.treeCaches && !reset.cachedBodyChars && !reset.activeBodyChars, 'Workspace caches survived reset');
    await phase(cycle, 'reset');
  }
}
