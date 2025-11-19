import React, { useState, useEffect } from 'react';
import { useBitcoinBrowser } from '../hooks/useBitcoinBrowser';
import type { AddressData } from '../types/address';

const AddressManager: React.FC = () => {
  const { generateAddress } = useBitcoinBrowser();
  const [addresses, setAddresses] = useState<AddressData[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleGenerateAddress = async () => {
    setLoading(true);
    setError(null);

    try {
      console.log('🔍 Generating new address...');

      // Add visible debug logging
      const debugDiv = document.getElementById('debug-log');
      if (debugDiv) {
        debugDiv.innerHTML += '🔍 Frontend: Starting address generation<br>';
      }

      // Check if bitcoinBrowser is available
      if (!window.bitcoinBrowser) {
        throw new Error('bitcoinBrowser not available');
      }
      if (!window.bitcoinBrowser.address) {
        throw new Error('bitcoinBrowser.address not available');
      }
      if (!window.bitcoinBrowser.address.generate) {
        throw new Error('bitcoinBrowser.address.generate not available');
      }

      if (debugDiv) {
        debugDiv.innerHTML += '🔍 Frontend: Calling bitcoinBrowser.address.generate()<br>';
      }

      const newAddress = await generateAddress();
      console.log('✅ Address generated:', newAddress);

      if (debugDiv) {
        debugDiv.innerHTML += '✅ Frontend: Address generated successfully<br>';
      }

      setAddresses(prev => [...prev, newAddress]);
    } catch (err) {
      console.error('❌ Error generating address:', err);

      // Add visible debug logging for errors
      const debugDiv = document.getElementById('debug-log');
      if (debugDiv) {
        debugDiv.innerHTML += '❌ Frontend: Error - ' + (err instanceof Error ? err.message : 'Unknown error') + '<br>';
      }

      setError(err instanceof Error ? err.message : 'Failed to generate address');
    } finally {
      setLoading(false);
    }
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text).then(() => {
      console.log('📋 Address copied to clipboard');
    }).catch(err => {
      console.error('❌ Failed to copy to clipboard:', err);
    });
  };

  return (
    <div className="address-manager">
      <h2>Address Manager</h2>

      <div className="address-actions">
        <button
          onClick={handleGenerateAddress}
          disabled={loading}
          className="generate-btn"
        >
          {loading ? 'Generating...' : 'Generate New Address'}
        </button>
      </div>

      {error && (
        <div className="error-message">
          ❌ {error}
        </div>
      )}

      <div className="addresses-list">
        <h3>Generated Addresses ({addresses.length})</h3>

        {addresses.length === 0 ? (
          <p className="no-addresses">No addresses generated yet. Click "Generate New Address" to create one.</p>
        ) : (
          <div className="addresses">
            {addresses.map((address, index) => {
              console.log(`🔍 Address ${index}:`, address);
              console.log(`🔍 Address.address:`, address.address);
              console.log(`🔍 Address type:`, typeof address.address);
              return (
                <div key={index} className="address-item">
                  <div className="address-header">
                    <span className="address-index">#{index + 1}</span>
                    <button
                      onClick={() => copyToClipboard(address.address)}
                      className="copy-btn"
                      title="Copy address"
                    >
                      📋
                    </button>
                  </div>
                  <div className="address-value" title={address.address}>
                    {address.address || 'NO ADDRESS FOUND'}
                  </div>
                  <div className="address-details">
                    <small>Public Key: {address.publicKey.substring(0, 20)}...</small>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <style>{`
        .address-manager {
          padding: 20px;
          max-width: 800px;
          margin: 0 auto;
        }

        .address-actions {
          margin-bottom: 20px;
        }

        .generate-btn {
          background: #007bff;
          color: white;
          border: none;
          padding: 12px 24px;
          border-radius: 6px;
          cursor: pointer;
          font-size: 16px;
          transition: background 0.2s;
        }

        .generate-btn:hover:not(:disabled) {
          background: #0056b3;
        }

        .generate-btn:disabled {
          background: #6c757d;
          cursor: not-allowed;
        }

        .error-message {
          background: #f8d7da;
          color: #721c24;
          padding: 12px;
          border-radius: 6px;
          margin-bottom: 20px;
          border: 1px solid #f5c6cb;
        }

        .addresses-list h3 {
          margin-bottom: 15px;
          color: #333;
        }

        .no-addresses {
          color: #6c757d;
          font-style: italic;
          text-align: center;
          padding: 40px;
        }

        .addresses {
          display: flex;
          flex-direction: column;
          gap: 15px;
        }

        .address-item {
          background: #f8f9fa;
          border: 1px solid #dee2e6;
          border-radius: 8px;
          padding: 15px;
          transition: box-shadow 0.2s;
        }

        .address-item:hover {
          box-shadow: 0 2px 8px rgba(0,0,0,0.1);
        }

        .address-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 10px;
        }

        .address-index {
          background: #007bff;
          color: white;
          padding: 4px 8px;
          border-radius: 4px;
          font-size: 12px;
          font-weight: bold;
        }

        .copy-btn {
          background: none;
          border: none;
          cursor: pointer;
          font-size: 16px;
          padding: 4px;
          border-radius: 4px;
          transition: background 0.2s;
        }

        .copy-btn:hover {
          background: #e9ecef;
        }

        .address-value {
          font-family: 'Courier New', monospace;
          font-size: 14px;
          word-break: break-all;
          background: white;
          color: #333;
          padding: 8px;
          border-radius: 4px;
          border: 1px solid #dee2e6;
          margin-bottom: 8px;
        }

        .address-details {
          color: #6c757d;
          font-size: 12px;
        }
      `}</style>
    </div>
  );
};

export default AddressManager;
