import React from 'react';
import { createPortal } from 'react-dom';

interface SimplePanelProps {
  isOpen: boolean;
  onClose: () => void;
}

const SimplePanel: React.FC<SimplePanelProps> = ({ isOpen, onClose }) => {
  console.log('🔴 SimplePanel render - isOpen:', isOpen);
  if (!isOpen) return null;

  // Use React Portal to render directly into document.body
  // This bypasses CEF's z-index limitations
  return createPortal(
    <>
      {/* Backdrop overlay */}
      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        width: '100vw',
        height: '100vh',
        background: 'rgba(255, 0, 0, 0.8)', // Bright red for debugging
        zIndex: 2147483647, // Maximum z-index value
        backdropFilter: 'blur(2px)'
      }} onClick={onClose} />

      {/* Panel */}
      <div style={{
      position: 'fixed',
      top: 0,
      right: 0,
      width: '400px',
      height: '100vh',
      background: '#2d5016', // Dark green
      color: 'white',
      zIndex: 2147483647, // Maximum z-index value (same as CEF)
      transform: 'translateX(0)',
      transition: 'transform 300ms ease-in-out',
      boxShadow: '-4px 0 20px rgba(0,0,0,0.2)',
      padding: '20px',
      display: 'flex',
      flexDirection: 'column',
      isolation: 'isolate', // Creates new stacking context
      willChange: 'transform' // Optimizes for animations
    }}>
      <div style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        marginBottom: '20px',
        borderBottom: '1px solid #d4c4a8',
        paddingBottom: '10px'
      }}>
        <h2 style={{ margin: 0, fontSize: '1.5em' }}>Test Panel</h2>
        <button
          onClick={onClose}
          style={{
            background: 'none',
            border: 'none',
            color: 'white',
            fontSize: '1.8em',
            cursor: 'pointer',
            padding: '5px 10px',
            borderRadius: '4px',
            transition: 'background-color 0.2s'
          }}
          onMouseOver={(e) => e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)'}
          onMouseOut={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
        >
          ×
        </button>
      </div>

      <div style={{ flex: 1 }}>
        <p>This is a simple test panel to verify:</p>
        <ul>
          <li>Panel appears/disappears correctly</li>
          <li>Z-index works (appears above everything)</li>
          <li>No layout shifts occur</li>
          <li>Close button works</li>
        </ul>

        <div style={{
          marginTop: '20px',
          padding: '15px',
          background: 'rgba(255,255,255,0.1)',
          borderRadius: '8px',
          border: '1px solid #d4c4a8'
        }}>
          <h4 style={{ margin: '0 0 10px 0' }}>Panel Status:</h4>
          <p style={{ margin: 0 }}>✅ Panel is open and visible</p>
        </div>
      </div>
    </div>
    </>,
    document.body // Portal target - renders directly into document.body
  );
};

export default SimplePanel;
