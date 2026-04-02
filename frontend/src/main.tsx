import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import App from './App';
import './bridge/initWindowBridge';

console.log("🎬 React started in", window.location.pathname.includes('overlay') ? '🪟 Overlay mode' : '🧭 Header mode');

ReactDOM.createRoot(document.getElementById('root')!).render(
  <BrowserRouter>
    <App />
  </BrowserRouter>
);

// Remove splash screen for overlay routes immediately (they need transparent bg).
// For the main header route (/), splash stays until MainBrowserView calls removeSplash().
const splash = document.getElementById('splash');
if (splash && window.location.pathname !== '/' && window.location.pathname !== '') {
  splash.classList.add('fade-out');
  splash.addEventListener('transitionend', () => splash.remove());
  setTimeout(() => splash.remove(), 300);
}

// Expose a global so MainBrowserView can remove splash after it renders
(window as any).removeSplash = () => {
  const s = document.getElementById('splash');
  if (s) {
    s.classList.add('fade-out');
    s.addEventListener('transitionend', () => s.remove());
    setTimeout(() => s.remove(), 300);
  }
};
