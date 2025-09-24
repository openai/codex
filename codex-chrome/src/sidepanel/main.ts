/**
 * Side panel main entry point
 */

import './sidepanel.css';
import App from './App.svelte';

const app = new App({
  target: document.getElementById('app')!,
});

export default app;
