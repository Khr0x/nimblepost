import { mount, tick } from 'svelte';
import App from './App.svelte';
import './styles/app.css';
import { applyTheme, readTheme } from '$lib/layout/theme';
import { recordStage } from '$lib/desktop/native-profile';
applyTheme(readTheme());
if (import.meta.env.VITE_NATIVE_VALIDATION === '1') performance.mark('nimblepost:frontend-start');
mount(App, { target: document.getElementById('app')! });
if (import.meta.env.VITE_NATIVE_VALIDATION === '1') {
  const started = performance.getEntriesByName('nimblepost:frontend-start')[0].startTime;
  void tick().then(() => recordStage('initial-dom-flush', started));
}
