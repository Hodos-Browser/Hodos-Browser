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
