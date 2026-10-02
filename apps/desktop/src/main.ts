import { mount } from 'svelte';
import App from './App.svelte';
import './styles/app.css';
import { applyTheme, readTheme } from '$lib/layout/theme';
applyTheme(readTheme());
mount(App, { target: document.getElementById('app')! });
