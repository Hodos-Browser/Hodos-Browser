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
  const { balance, usdValue, isLoading: balanceLoading, refreshBalance } = useBalance();
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
          <span className="balance-title">Balance</span>
          <button
            className="refresh-button-light"
            onClick={refreshBalance}
            disabled={balanceLoading}
            title="Refresh balance"
          >
            {balanceLoading ? '...' : 'Refresh'}
          </button>
        </div>
        <div className="balance-content-light">
          <div className="balance-primary-light">
            <span className="balance-amount-light">
              ${balanceLoading ? '...' : usdValue.toFixed(2)}
            </span>
            <span className="balance-currency-light">USD</span>
          </div>
          <div className="balance-secondary-light">
            <span className="balance-usd-light">
              {balanceLoading ? '...' : (balance / 100000000).toFixed(8)} BSV
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
          <div className="send-form-container-light">
            <TransactionForm
              onTransactionCreated={handleSendSubmit}
              balance={balance}
            />
          </div>
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
        fullWidth
        size="small"
        sx={{
          fontSize: '12px',
          marginTop: '8px',
          borderColor: '#2d5016',
          color: '#2d5016',
          '&:hover': {
            borderColor: '#3a641e',
            backgroundColor: 'rgba(45, 80, 22, 0.04)',
          }
        }}
      >
        Advanced Wallet
      </Button>
    </div>
  );
}
