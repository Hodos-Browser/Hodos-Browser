import { useMemo } from 'react';
import WalletPanel from '../components/WalletPanel';

export default function WalletPanelPage() {
  // Read icon position from URL param (physical pixels, passed from toolbar click)
  const paddingRightPx = useMemo(() => {
    const params = new URLSearchParams(window.location.search);
    const iro = parseInt(params.get('iro') || '0', 10);
    if (iro <= 0) return 0;
    const dpr = window.devicePixelRatio || 1;
    return Math.round(iro / dpr);
  }, []);

  const handleClose = () => {
    if (window.hodosBrowser?.overlay?.close) {
      window.hodosBrowser.overlay.close();
    } else if (window.cefMessage?.send) {
      window.cefMessage.send('overlay_close', []);
    }
  };

  const handleBackgroundClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      handleClose();
    }
  };

  return (
    <div
      onClick={handleBackgroundClick}
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100vw',
        height: '100vh',
        margin: 0,
        padding: 0,
        overflow: 'hidden',
        display: 'flex',
        justifyContent: 'flex-end',
        alignItems: 'flex-start',
        paddingTop: '150px',
        paddingRight: paddingRightPx > 0 ? `${paddingRightPx}px` : '0px',
        boxSizing: 'border-box',
        cursor: 'pointer',
        backgroundColor: 'rgba(0, 0, 0, 0.01)',
      }}
    >
      <WalletPanel onClose={handleClose} />
    </div>
  );
}
