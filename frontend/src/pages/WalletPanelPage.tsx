import WalletPanel from '../components/WalletPanel';

export default function WalletPanelPage() {
  const handleClose = () => {
    console.log('🔧 WalletPanelPage: Closing wallet panel overlay');
    console.log('🔧 window.cefMessage:', window.cefMessage);
    console.log('🔧 window.hodosBrowser?.overlay:', window.hodosBrowser?.overlay);

    // Try the hodosBrowser.overlay.close method first (most reliable)
    if (window.hodosBrowser?.overlay?.close) {
      console.log('🔧 Using window.hodosBrowser.overlay.close()');
      window.hodosBrowser.overlay.close();
    } else if (window.cefMessage?.send) {
      console.log('🔧 Using window.cefMessage.send()');
      window.cefMessage.send('overlay_close', []);
    } else {
      console.error('❌ No close method available!');
    }
  };

  const handleBackgroundClick = (e: React.MouseEvent) => {
    console.log('🔧 Background clicked, target:', e.target, 'currentTarget:', e.currentTarget);
    // Only close if clicking the background, not the panel itself
    if (e.target === e.currentTarget) {
      console.log('🔧 Click was on background, calling handleClose()');
      handleClose();
    } else {
      console.log('🔧 Click was on panel content, ignoring');
    }
  };

  return (
    <div
      onClick={handleBackgroundClick}
      style={{
        width: '100%',
        height: '100%',
        margin: 0,
        padding: 0,
        overflow: 'hidden',
        display: 'flex',
        justifyContent: 'flex-end',    // Align panel to right
        alignItems: 'flex-start',      // Keep at top
        paddingTop: '60px',            // Space from top (below toolbar)
        paddingRight: '16px',          // Space from right edge
        boxSizing: 'border-box',
        cursor: 'pointer',             // Indicate clickable background
      }}
    >
      <WalletPanel onClose={handleClose} />
    </div>
  );
}
