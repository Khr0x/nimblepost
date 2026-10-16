// Loaded only by the validation build. Actions use the same handlers as the UI.
import { emit } from '@tauri-apps/api/event';
import { tick } from 'svelte';
import type { MemoryControls } from './native-validation';
import { stopProfiling, profileResponses, takeResponseStages } from './native-profile';
import { api, traceNativeIpc } from './api';

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const frames = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
const ensure = (ok: unknown, message: string) => { if (!ok) throw new Error(message); };
const digest = async (text: string) => [...new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text)))].map(byte => byte.toString(16).padStart(2, '0')).join('');

export async function validateResponses(controls: MemoryControls) {
  stopProfiling();
  if (controls.responseDiagnostics) traceNativeIpc();
  const waitForHeapProfile = async () => {
    const deadline = performance.now() + 90000;
    while (!(await api.workspace()).validationPlan?.profileReady) {
      ensure(performance.now() < deadline, 'WebKit heap profiler did not become ready');
      await delay(250);
    }
  };
  if (controls.fineProfile) profileResponses();
  const clearIpc = () => { for (const entry of performance.getEntriesByType('measure')) if (entry.name.startsWith('native-ipc:')) performance.clearMeasures(entry.name); };
  const send = async (url: string, raw = false) => {
    await controls.useEnvironment();
    controls.provideSecret('np013-token', 'NP013_RUNTIME_SECRET_CANARY');
    return raw ? controls.sendRaw(url) : controls.send(url);
  };
  let index = 0;
  let actions: { name: string; durationMs: number; handlerMs: number; flushMs: number; frameMs: number; ipc?: { name: string; startMs: number; durationMs: number }[] }[] = [];
  const wait = async (condition: () => boolean, message: string) => {
    const deadline = performance.now() + 15000;
    while (!condition()) { ensure(performance.now() < deadline, message); await delay(10); }
  };
  const clean = () => {
    const state = controls.inspect();
    ensure(!state.error && !state.recoveryWarning, state.error || state.recoveryWarning);
    return state;
  };
  const measureAction = async (name: string, perform: () => Promise<void> | void, visible: () => boolean) => {
    clearIpc();
    const started = performance.now();
    await perform(); const handled = performance.now();
    await tick(); const flushed = performance.now();
    await frames();
    ensure(visible(), `Action did not become visible: ${name}`);
    const visibleAt = performance.now();
    const ipc = controls.responseDiagnostics ? performance.getEntriesByType('measure').filter(entry => entry.name.startsWith('native-ipc:') && entry.startTime >= started)
      .map(entry => ({ name: entry.name.slice('native-ipc:'.length), startMs: entry.startTime - started, durationMs: entry.duration })) : undefined;
    const result = { name, durationMs: visibleAt - started, handlerMs: handled - started, flushMs: flushed - handled, frameMs: visibleAt - flushed, ...(ipc ? { ipc } : {}) };
    clearIpc();
    if (!controls.fineProfile) return result;
    // Inspect after the measured interval; retain CSS property names, never values or page text.
    const animations = document.getAnimations().filter(animation => animation.playState === 'running');
    return { ...result, animations: { dropped: Math.max(0, animations.length - 128), active: animations.slice(0, 128).map(animation => {
      const effect = animation.effect;
      const target = effect instanceof KeyframeEffect && effect.target instanceof Element ? effect.target : null;
      const area = target?.closest('.request-tabs') ? 'tabs' : target?.closest('aside') ? 'sidebar' : target?.closest('main') ? 'workbench' : 'other';
      const properties = effect instanceof KeyframeEffect ? [...new Set(effect.getKeyframes().flatMap(frame => Object.keys(frame)))].filter(key => !['offset', 'computedOffset', 'easing', 'composite'].includes(key)) : [];
      return { area, properties };
    }) } };
  };
  const action = async (name: string, perform: () => Promise<void> | void, visible: () => boolean) => { actions.push(await measureAction(name, perform, visible)); };
  const phase = async (cycle: number, name: string, extra: Record<string, unknown> = {}, settle = true) => {
    await tick(); await frames();
    const controlsTiming = [];
    if (controls.responseDiagnostics) {
      for (let i = 0; i < 5; i++) {
        let start = performance.now(); await delay(0); const timerMs = performance.now() - start;
        start = performance.now(); await frames(); const framesMs = performance.now() - start;
        start = performance.now(); await api.recovery(); const readRecoveryMs = performance.now() - start;
        controlsTiming.push({ timerMs, framesMs, readRecoveryMs });
      }
    }
    clearIpc();
    const state = controls.inspect();
    ensure(!state.recoveryWarning, state.recoveryWarning);
    const frontend = controls.fineProfile ? takeResponseStages() : undefined;
    if (frontend && name === 'json') {
      const indexed = frontend.stages.find(stage => stage.name === 'response-index')?.count ?? 0;
      const geometry = frontend.stages.find(stage => stage.name === 'response-tops')?.count ?? 0;
      ensure(indexed > 0 && geometry <= indexed + 12, 'JSON appends rebuilt unused geometry tables');
    }
    await emit('native-validation-responses', { index: index++, cycle, name, ...state, actions,
      ...(controls.responseDiagnostics ? { diagnostics: { controls: controlsTiming, visibility: document.visibilityState, focused: document.hasFocus(), ...(frontend ? { frontend } : {}) } } : {}), ...extra });
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
    await phase(cycle, 'copy', { kind, content: await content(), point: { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2, innerHeight: window.innerHeight, innerWidth: window.innerWidth } }, false);
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
  for (let cycle = 1; cycle <= (controls.fineProfile ? 1 : 3); cycle++) {
    if (cycle > 1) await controls.reset(1);
    await controls.open('request-00000.yml');
    for (let request = 1; request < 64; request++) await controls.open(`request-${String(request).padStart(5, '0')}.yml`);
    ensure(clean().tabs.length === 64, 'Expected 64 tabs');
    await tick(); await frames();
    const group = document.querySelector<HTMLFieldSetElement>('fieldset.request-tabs-track');
    ensure(group, 'Native tab group is missing');
    await wait(() => !group!.disabled, `Native tab group stayed disabled: ${JSON.stringify(controls.inspect())}`);
    const blockedTab = group!.querySelector<HTMLButtonElement>('.request-tab-trigger')!;
    const urlInput = document.querySelector<HTMLInputElement>('input[aria-label="Request URL"]');
    urlInput!.focus();
    let clicked = false;
    const clickedWhileDisabled = () => { clicked = true; };
    blockedTab.addEventListener('click', clickedWhileDisabled);
    group!.disabled = true;
    ensure(blockedTab.matches(':disabled'), 'Tabs must inherit native disabled state');
    blockedTab.focus(); ensure(document.activeElement !== blockedTab, 'Disabled tabs received keyboard focus');
    blockedTab.click(); group!.disabled = false;
    blockedTab.removeEventListener('click', clickedWhileDisabled);
    ensure(!clicked, 'A disabled tab received a native click');
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
    if (controls.heapProfile) await waitForHeapProfile();
    await send(`${controls.url}/json`); await complete(); clean();
    ensure(document.querySelector('.response-body-note')?.textContent?.includes('too large'), 'Large JSON must retain explicit formatting fallback');
    await phase(cycle, 'json', { content: await content(), renderedRows: document.querySelectorAll('[data-response-line]').length });
    await copy(cycle, 'json');
    await send(`${controls.url}/line`);
    ensure(controls.inspect().activeOffset === 65536, 'Partial response must stop inside the fixture emoji');
    await action('tab-empty-loading', () => controls.activate(tabs[0]), () => !document.querySelector('.response-panel'));
    await action('tab-partial', () => controls.activate(tabs.at(-1)!), () => controls.inspect().activeOffset === 65536 && !!document.querySelector('.body-more'));
    await complete(); clean();
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
    const activeButton = document.querySelector<HTMLElement>('.request-document.active .request-tab-trigger');
    const tabViewport = document.querySelector<HTMLElement>('.request-tabs-list');
    const activeRect = activeButton!.getBoundingClientRect(), tabRect = tabViewport!.getBoundingClientRect();
    ensure(activeRect.left >= tabRect.left - 1 && activeRect.right <= tabRect.right + 1, 'Active tab is outside its scroll viewport');
    ensure(await digest(controls.body()) === lineHash, 'Tab switch changed the response');
    const uiProfile = [];
    if (controls.fineProfile) {
      const priorStages = takeResponseStages();
      uiProfile.push({ mode: 'regular-actions', frontend: priorStages });
      const style = document.createElement('style'); document.head.append(style);
      const modes = [
        ['baseline', ''],
        ['no-transitions', '.app-shell * { transition: none !important; animation: none !important; }'],
        ['system-font', '.app-shell { font-family: system-ui, sans-serif !important; }'],
        ['contained', '.response-text-viewport { contain: layout paint; }'],
        ['hidden-tabs', '.request-tabs { display: none !important; }'],
        ['no-button-opacity-transition', '.app-shell button { transition-property: color, background-color, border-color, box-shadow, transform !important; }'],
        ['group-disabled-opacity', '.request-tabs-track button:disabled { opacity: 1 !important; } .request-tabs-track:disabled { opacity: 0.5; }'],
        ['baseline-after', ''],
      ];
      try {
        for (const [mode, css] of modes) {
          style.textContent = css; await tick(); await frames(); await delay(150);
          for (let repeat = 0; repeat < 3; repeat++) {
            takeResponseStages();
            const empty = await measureAction('tab-empty', () => controls.activate(tabs[0]), () => !document.querySelector('.response-panel'));
            const restored = await measureAction('tab-response', () => controls.activate(tabs.at(-1)!), () => !!document.querySelector('.response-panel'));
            const view = inspectView();
            const scroll = await measureAction('scroll', () => { view.scrollTop = view.scrollHeight; view.dispatchEvent(new Event('scroll')); }, () => !!document.querySelector(`[data-response-line="${Number(view.dataset.responseLines) - 1}"]`));
            uiProfile.push({ mode, repeat, actions: [empty, restored, scroll], frontend: takeResponseStages() });
          }
        }
      } finally { style.remove(); }
      ensure(await digest(controls.body()) === lineHash, 'Profiling changed response content');
    }
    await phase(cycle, 'line', { content: await content(), ...(controls.fineProfile ? { uiProfile } : {}) });
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
  if (controls.responseDiagnostics) {
    // Observe normal idle reclamation after loop-local DOM references leave scope.
    await delay(30000);
    await phase(controls.fineProfile ? 1 : 3, 'idle-reset');
  }
}
