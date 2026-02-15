import { useState } from 'react';
import { Button } from '@mui/material';
import SettingsIcon from '@mui/icons-material/Settings';
import { TransactionForm } from './TransactionForm';
import { useBalance } from '../hooks/useBalance';
import { useAddress } from '../hooks/useAddress';
import type { TransactionResponse } from '../types/transaction';
import './TransactionComponents.css';
import './WalletPanel.css';

interface WalletPanelProps {
  onClose?: () => void;
}

export default function WalletPanel({ onClose }: WalletPanelProps) {
  const { balance, usdValue, isLoading, isRefreshing, refreshBalance } = useBalance();
  const { currentAddress, isGenerating, generateAndCopy } = useAddress();

  const [showSendForm, setShowSendForm] = useState(false);
  const [transactionResult, setTransactionResult] = useState<TransactionResponse | null>(null);
  const [showReceiveAddress, setShowReceiveAddress] = useState(false);
  const [addressCopiedMessage, setAddressCopiedMessage] = useState<string | null>(null);

  // State for button click animations
  const [clickedButtons, setClickedButtons] = useState<Set<string>>(new Set());
  const [copyAgainClicked, setCopyAgainClicked] = useState(false);
  const [copyLinkClicked, setCopyLinkClicked] = useState(false);

  const handleSendClick = () => {
    // Clear all other display states first
    setShowReceiveAddress(false);
    setAddressCopiedMessage(null);
    setTransactionResult(null);

    // Toggle send form
    setShowSendForm(!showSendForm);
  };

  const handleReceiveClick = async () => {
    console.log('Receive button clicked');

    // Immediately show visual feedback - keep clicked state until operation completes
    setClickedButtons(prev => new Set(prev).add('receive'));

    // Clear all other display states first
    setShowSendForm(false);
    setTransactionResult(null);

    try {
      // Generate address from identity
      const addressData = await generateAndCopy();
      console.log('Address generated and copied:', addressData);

      setShowReceiveAddress(true);
      setAddressCopiedMessage(`Address copied to clipboard: ${addressData.substring(0, 10)}...`);

      // Clear the message after 3 seconds
      setTimeout(() => {
        setAddressCopiedMessage(null);
      }, 3000);
    } catch (error) {
      console.error('Failed to generate address:', error);
      setAddressCopiedMessage(`Error: ${error instanceof Error ? error.message : 'Unknown error'}`);
    } finally {
      // Remove clicked state after operation completes (with small delay for visual feedback)
      setTimeout(() => {
        setClickedButtons(prev => {
          const newSet = new Set(prev);
          newSet.delete('receive');
          return newSet;
        });
      }, 200);
    }
  };

  const handleSendSubmit = (result: TransactionResponse) => {
    // Clear all other states first
    setShowReceiveAddress(false);
    setAddressCopiedMessage(null);
    setTransactionResult(null);

    // Set the transaction result and close the form
    setTransactionResult(result);
    setShowSendForm(false);

    // Only refresh balance if transaction was successful
    if (result.success !== false && result.status !== 'failed') {
      refreshBalance();
    }
  };

  const handleCopyAgain = async () => {
    try {
      await navigator.clipboard.writeText(currentAddress || '');
      setCopyAgainClicked(true);
      setTimeout(() => setCopyAgainClicked(false), 2000);
    } catch (error) {
      console.error('Failed to copy address:', error);
    }
  };

  const handleCopyLink = async () => {
    if (transactionResult?.whatsOnChainUrl) {
      try {
        await navigator.clipboard.writeText(transactionResult.whatsOnChainUrl);
        setCopyLinkClicked(true);
        setTimeout(() => setCopyLinkClicked(false), 2000);
      } catch (error) {
        console.error('Failed to copy link:', error);
      }
    }
  };

  const handleAdvanced = () => {
    console.log('Advanced button clicked - opening wallet page in new tab');
    // Open wallet page in new tab
    if (window.cefMessage) {
      window.cefMessage.send('tab_create', 'http://127.0.0.1:5137/wallet');
    }
  };

  return (
    <div className="wallet-panel-light" onClick={(e) => e.stopPropagation()}>
      {/* Balance Display */}
      <div className="balance-display-light">
        <div className="balance-header-light">
          <div className="balance-brand">
            <svg className="balance-logo" viewBox="0 0 216 216" xmlns="http://www.w3.org/2000/svg">
              <defs>
                <linearGradient id="bg1" x1="129.34" y1="157.77" x2="77.67" y2="169.6" gradientTransform="translate(0 218) scale(1 -1)" gradientUnits="userSpaceOnUse"><stop offset="0" stopColor="#fff"/><stop offset="1" stopColor="#a67c00"/></linearGradient>
                <linearGradient id="bg2" x1="156.87" y1="128.68" x2="128.7" y2="173.59" gradientTransform="translate(0 218) scale(1 -1)" gradientUnits="userSpaceOnUse"><stop offset="0" stopColor="#fff"/><stop offset="1" stopColor="#a67c00"/></linearGradient>
                <linearGradient id="bg3" x1="155.76" y1="88.66" x2="167.6" y2="140.33" gradientTransform="translate(0 218) scale(1 -1)" gradientUnits="userSpaceOnUse"><stop offset="0" stopColor="#fff"/><stop offset="1" stopColor="#a67c00"/></linearGradient>
                <linearGradient id="bg4" x1="126.69" y1="61.13" x2="171.6" y2="89.32" gradientTransform="translate(0 218) scale(1 -1)" gradientUnits="userSpaceOnUse"><stop offset="0" stopColor="#fff"/><stop offset="1" stopColor="#a67c00"/></linearGradient>
                <linearGradient id="bg5" x1="86.66" y1="62.21" x2="138.35" y2="50.39" gradientTransform="translate(0 218) scale(1 -1)" gradientUnits="userSpaceOnUse"><stop offset="0" stopColor="#fff"/><stop offset="1" stopColor="#a67c00"/></linearGradient>
                <linearGradient id="bg6" x1="59.15" y1="91.32" x2="87.31" y2="46.42" gradientTransform="translate(0 218) scale(1 -1)" gradientUnits="userSpaceOnUse"><stop offset="0" stopColor="#fff"/><stop offset="1" stopColor="#a67c00"/></linearGradient>
                <linearGradient id="bg7" x1="60.2" y1="131.35" x2="48.38" y2="79.67" gradientTransform="translate(0 218) scale(1 -1)" gradientUnits="userSpaceOnUse"><stop offset="0" stopColor="#fff"/><stop offset="1" stopColor="#a67c00"/></linearGradient>
                <linearGradient id="bg8" x1="89.31" y1="158.88" x2="44.4" y2="130.72" gradientTransform="translate(0 218) scale(1 -1)" gradientUnits="userSpaceOnUse"><stop offset="0" stopColor="#fff"/><stop offset="1" stopColor="#a67c00"/></linearGradient>
              </defs>
              <path fill="url(#bg1)" d="M73.39,93.44c3.75-8.91,10.9-16.33,20.45-20.21,11.8-15.48,26.02-24.5,39.33-29.7-8.72-21.78-26.55-34.52-26.55-34.52,0,0-16.35,12.7-26.4,35.61-5.75,13.09-9.42,29.53-6.81,48.82h-.01Z"/>
              <path fill="url(#bg2)" d="M93.84,73.21c.56-.23,1.11-.49,1.69-.69,9.16-3.22,18.71-2.63,27.02.88,19.31-2.61,35.74,1.06,48.84,6.81,9.23-21.57,5.62-43.19,5.62-43.19,0,0-20.54-2.57-43.85,6.52-13.32,5.2-27.53,14.22-39.32,29.7v-.03Z"/>
              <path fill="url(#bg3)" d="M207,106.61s-12.71-16.35-35.61-26.4c-13.1-5.75-29.54-9.42-48.84-6.81,8.91,3.75,16.33,10.89,20.21,20.43,15.48,11.8,24.52,26.01,29.71,39.34,21.78-8.73,34.53-26.56,34.53-26.56Z"/>
              <path fill="url(#bg4)" d="M142.77,93.84c.23.58.5,1.13.7,1.7,3.12,8.9,2.78,18.41-.87,27.03,2.6,19.29-1.06,35.72-6.81,48.81,21.57,9.23,43.19,5.62,43.19,5.62,0,0,2.57-20.54-6.52-43.85-5.2-13.32-14.22-27.54-29.71-39.34h.01Z"/>
              <path fill="url(#bg5)" d="M142.61,122.57c-.23.56-.45,1.14-.71,1.7-4.08,8.5-11.06,14.99-19.73,18.51-11.8,15.48-26.01,24.5-39.32,29.7,8.73,21.78,26.56,34.52,26.56,34.52,0,0,16.35-12.7,26.4-35.61,5.75-13.09,9.42-29.52,6.81-48.81h0Z"/>
              <path fill="url(#bg6)" d="M122.16,142.77c-.56.23-1.13.49-1.7.7-4.11,1.44-8.29,2.13-12.42,2.13-5.07,0-10.05-1.06-14.63-3-19.28,2.6-35.69-1.07-48.78-6.82-9.23,21.57-5.62,43.19-5.62,43.19,0,0,20.54,2.57,43.85-6.52,13.32-5.2,27.53-14.22,39.32-29.7h-.01Z"/>
              <path fill="url(#bg7)" d="M93.4,142.6c-8.91-3.77-16.34-10.93-20.21-20.47-15.45-11.8-24.47-25.99-29.66-39.29-21.78,8.72-34.53,26.55-34.53,26.55,0,0,12.7,16.35,35.61,26.4,13.09,5.75,29.51,9.4,48.79,6.82Z"/>
              <path fill="url(#bg8)" d="M73.19,122.13c-.22-.55-.47-1.1-.66-1.68-3.22-9.16-2.63-18.71.88-27.02-2.61-19.29,1.06-35.73,6.81-48.82-21.57-9.23-43.19-5.62-43.19-5.62,0,0-2.57,20.54,6.52,43.85,5.2,13.31,14.2,27.51,29.66,39.29h-.01Z"/>
              <path fill="#a57d2d" d="M95.54,72.53c-.58.21-1.13.47-1.69.69-9.54,3.88-16.69,11.3-20.45,20.21-3.51,8.3-4.1,17.86-.88,27.02.21.58.44,1.11.66,1.68,3.88,9.56,11.3,16.71,20.21,20.47,4.59,1.94,9.56,3,14.63,3,4.12,0,8.32-.69,12.42-2.13.58-.21,1.14-.47,1.7-.7,8.68-3.52,15.65-10.01,19.73-18.51.27-.56.48-1.13.71-1.7,3.64-8.62,3.99-18.13.87-27.03-.21-.59-.47-1.14-.7-1.7-3.88-9.54-11.3-16.68-20.21-20.43-8.31-3.49-17.86-4.08-27.02-.88v.03ZM138.88,97.15c6,17.05-2.98,35.73-20.03,41.73s-35.74-2.98-41.73-20.03c-6-17.05,2.98-35.73,20.03-41.73,17.05-5.99,35.74,2.98,41.73,20.03Z"/>
            </svg>
            <div className="balance-brand-text">
              <span className="brand-hodos">HODOS</span>
              <span className="brand-wallet">WALLET</span>
            </div>
          </div>
          <button
            className="refresh-button-light"
            onClick={refreshBalance}
            disabled={isRefreshing}
            title="Refresh balance"
          >
            {isRefreshing ? 'Refreshing...' : 'Refresh'}
          </button>
        </div>
        <div className="balance-content-light">
          <div className="balance-primary-light">
            <span className="balance-amount-light">
              {isLoading ? '...' : `$${usdValue.toFixed(2)}`}
            </span>
            <span className="balance-currency-light">USD</span>
          </div>
          <div className="balance-secondary-light">
            <span className="balance-usd-light">
              {isLoading ? '...' : (balance / 100000000).toFixed(8)} BSV
            </span>
          </div>
        </div>
      </div>

      {/* Action Buttons */}
      <div className="wallet-actions-light">
        <button
          className={`wallet-button-light receive-button-light ${clickedButtons.has('receive') || isGenerating ? 'clicked' : ''}`}
          onClick={handleReceiveClick}
          disabled={isGenerating}
        >
          {isGenerating ? 'Generating...' : 'Receive'}
        </button>
        <button
          className={`wallet-button-light send-button-light ${showSendForm ? 'active' : ''}`}
          onClick={handleSendClick}
        >
          {showSendForm ? 'Close' : 'Send'}
        </button>
      </div>

      {/* Dynamic Content Area */}
      <div className="dynamic-content-area-light">
        {showSendForm && (
          <TransactionForm
            onTransactionCreated={handleSendSubmit}
            balance={balance}
          />
        )}

        {showReceiveAddress && (
          <div className="receive-address-container-light">
            <h3>Receive Bitcoin SV</h3>
            <p>Address copied to clipboard!</p>
            <div className="address-display-light">
              <code>{currentAddress || 'Generating...'}</code>
            </div>
            <div className="address-buttons-light">
              <button
                className={`copy-button-light ${copyAgainClicked ? 'clicked' : ''}`}
                onClick={handleCopyAgain}
              >
                {copyAgainClicked ? 'Copied!' : 'Copy Again'}
              </button>
              <button
                className="close-button-light"
                onClick={() => setShowReceiveAddress(false)}
              >
                Close
              </button>
            </div>
          </div>
        )}

        {/* Success/Error Modal */}
        {transactionResult && (
          <div className={transactionResult.success === false || transactionResult.status === 'failed' ? 'error-message-light' : 'success-message-light'}>
            {transactionResult.success === false || transactionResult.status === 'failed' ? (
              <>
                <h3>Transaction Failed</h3>
                <div className="transaction-details-light">
                  <p><strong>Error:</strong> {transactionResult.error || transactionResult.message || 'Transaction broadcast failed'}</p>
                  {transactionResult.txid && (
                    <p><strong>TxID:</strong> {transactionResult.txid}</p>
                  )}
                </div>
                <button onClick={() => setTransactionResult(null)} className="close-button-light">
                  Close
                </button>
              </>
            ) : (
              <>
                <h3>Transaction Sent!</h3>
                <div className="transaction-details-light">
                  {transactionResult.txid && (
                    <p><strong>TxID:</strong> <span className="txid-display">{transactionResult.txid.substring(0, 16)}...</span></p>
                  )}
                  {transactionResult.message && (
                    <p><strong>Status:</strong> {transactionResult.message}</p>
                  )}
                </div>
                {transactionResult.whatsOnChainUrl && (
                  <div className="whatsonchain-container-light">
                    <a
                      href={transactionResult.whatsOnChainUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="whatsonchain-link-light"
                    >
                      View on WhatsOnChain
                    </a>
                    <button
                      onClick={handleCopyLink}
                      className={`copy-link-button-light ${copyLinkClicked ? 'clicked' : ''}`}
                    >
                      {copyLinkClicked ? 'Copied!' : 'Copy Link'}
                    </button>
                  </div>
                )}
                <button onClick={() => setTransactionResult(null)} className="close-button-light">
                  Close
                </button>
              </>
            )}
          </div>
        )}

        {!showSendForm && !showReceiveAddress && !transactionResult && (
          <div className="content-placeholder-light">
            {addressCopiedMessage ? (
              <div className="address-copied-message-light">
                {addressCopiedMessage}
              </div>
            ) : (
              <span className="placeholder-text">Click Send or Receive to get started</span>
            )}
          </div>
        )}
      </div>

      {/* Advanced Button */}
      <Button
        variant="outlined"
        startIcon={<SettingsIcon />}
        onClick={handleAdvanced}
        size="small"
        fullWidth
        sx={{
          marginTop: '8px',
          fontSize: '12px',
          borderColor: '#2d5016',
          color: '#2d5016',
          '&:hover': {
            borderColor: '#3a641e',
            backgroundColor: 'rgba(45, 80, 22, 0.04)',
          }
        }}
      >
        Advanced
      </Button>
    </div>
  );
}
