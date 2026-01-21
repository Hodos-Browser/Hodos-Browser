import React from 'react';
import WalletPanel from '../components/WalletPanel';

export default function WalletPanelPage() {
  return (
    <div style={{
      width: '100%',
      height: '100%',
      margin: 0,
      padding: 0,
      overflow: 'hidden'
    }}>
      <WalletPanel />
    </div>
  );
}
