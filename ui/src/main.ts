import { mount } from 'svelte';
import App from './App.svelte';
import './styles.css';

// Block the browser context menu and reload shortcuts outside text fields: this is a desktop app.
window.addEventListener('contextmenu', (event) => {
  const target = event.target as HTMLElement;
  if (!target.closest('input, textarea')) event.preventDefault();
});
window.addEventListener('keydown', (event) => {
  if (event.key === 'F5' || ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'r' && event.shiftKey)) event.preventDefault();
});

export default mount(App, { target: document.getElementById('app')! });
