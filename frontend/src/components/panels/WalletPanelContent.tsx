
import { useState, useRef } from 'react';
import { TransactionForm } from '../TransactionForm';
import { useBalance } from '../../hooks/useBalance';
import { useAddress } from '../../hooks/useAddress';
// Removed useWallet import - private keys handled by Go daemon
import type { TransactionResponse } from '../../types/transaction';
import '../../components/TransactionComponents.css';
import '../../components/WalletPanel.css';

const WalletPanel = () => {
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

  // Refs for animation timeouts
  const animationTimeouts = useRef<Map<string, NodeJS.Timeout>>(new Map());

  // No wallet initialization needed - using hardcoded test address

  // Helper function to trigger button click animation
  const triggerButtonAnimation = (buttonId: string, duration: number = 300) => {
    setClickedButtons(prev => new Set(prev).add(buttonId));

    // Clear existing timeout if any
    const existingTimeout = animationTimeouts.current.get(buttonId);
    if (existingTimeout) {
      clearTimeout(existingTimeout);
    }

    // Remove animation class after duration
    const timeout = setTimeout(() => {
      setClickedButtons(prev => {
        const newSet = new Set(prev);
        newSet.delete(buttonId);
        return newSet;
      });
      animationTimeouts.current.delete(buttonId);
    }, duration);

    animationTimeouts.current.set(buttonId, timeout);
  };

  const handleSendClick = () => {
    // Clear all other display states first
    setShowReceiveAddress(false);
    setAddressCopiedMessage(null);
    setTransactionResult(null);

    // Toggle send form
    setShowSendForm(!showSendForm);
  };

  const handleReceiveClick = async () => {
    console.log('🔄 Receive button clicked');

    // Immediately show visual feedback - keep clicked state until operation completes
    setClickedButtons(prev => new Set(prev).add('receive'));

    // Clear all other display states first
    setShowSendForm(false);
    setTransactionResult(null);

    try {
      // Generate address from identity
      const addressData = await generateAndCopy();
      console.log('✅ Address generated and copied:', addressData);

      setShowReceiveAddress(true);
      setAddressCopiedMessage(`Address copied to clipboard: ${addressData.substring(0, 10)}...`);
      console.log('✅ Message set:', `Address copied to clipboard: ${addressData.substring(0, 10)}...`);

      // Clear the message after 3 seconds
      setTimeout(() => {
        console.log('🔄 Clearing address copied message');
        setAddressCopiedMessage(null);
      }, 3000);
    } catch (error) {
      console.error('❌ Failed to generate address:', error);
      setAddressCopiedMessage(`Error: ${error instanceof Error ? error.message : 'Unknown error'}`);
    } finally {
      // Remove clicked state after operation completes (with small delay for visual feedback)
      setTimeout(() => {
        setClickedButtons(prev => {
          const newSet = new Set(prev);
          newSet.delete('receive');
          return newSet;
        });
      }, 200); // Small delay to show completion feedback
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


  const handleNavButtonClick = (buttonId: string) => {
    // Trigger animation
    triggerButtonAnimation(buttonId, 300);
    // Clear all states
    clearAllStates();
  };

  const clearAllStates = () => {
    setShowSendForm(false);
    setShowReceiveAddress(false);
    setAddressCopiedMessage(null);
    setTransactionResult(null);
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

  return (
    <div className="wallet-panel-container">
      {/* Balance Display */}
      <div className="balance-display">
          <div className="balance-header">
            <h2>Total Balance</h2>
            <button
              className="refresh-button"
              onClick={refreshBalance}
              disabled={isRefreshing}
            >
              {isRefreshing ? '⏳ Refreshing...' : '🔄 Refresh'}
            </button>
          </div>
          <div className="balance-content">
            <div className="balance-primary">
              <span className="balance-amount">
                {isLoading ? '...' : (balance / 100000000).toFixed(8)}
              </span>
              <span className="balance-currency">BSV</span>
            </div>
            <span className="balance-separator">|</span>
            <div className="balance-secondary">
              <span className="balance-usd">
                ${isLoading ? '...' : usdValue.toFixed(2)} USD
              </span>
            </div>
          </div>
          {isLoading && (
            <div className="balance-loading">
              <div className="loading-spinner"></div>
              Fetching balance from blockchain...
            </div>
          )}
        </div>

        {/* Action Buttons */}
        <div className="wallet-actions">
          <button
            className={`wallet-button receive-button ${clickedButtons.has('receive') || isGenerating ? 'clicked' : ''}`}
            onClick={handleReceiveClick}
            disabled={isGenerating}
          >
            {isGenerating ? 'Generating...' : 'Receive'}
          </button>
          <button
            className={`wallet-button send-button ${showSendForm ? 'active' : ''}`}
            onClick={handleSendClick}
          >
            {showSendForm ? 'Close Send' : 'Send'}
          </button>
        </div>

        {/* Navigation Grid */}
        <div className="navigation-grid">
          <button
            className={`nav-grid-button ${clickedButtons.has('certificates') ? 'clicked' : ''}`}
            onClick={() => handleNavButtonClick('certificates')}
          >
            Certificates
          </button>
          <button
            className={`nav-grid-button ${clickedButtons.has('history') ? 'clicked' : ''}`}
            onClick={() => handleNavButtonClick('history')}
          >
            History
          </button>
          <button
            className={`nav-grid-button ${clickedButtons.has('settings') ? 'clicked' : ''}`}
            onClick={() => handleNavButtonClick('settings')}
          >
            Settings
          </button>
          <button
            className={`nav-grid-button ${clickedButtons.has('tokens') ? 'clicked' : ''}`}
            onClick={() => handleNavButtonClick('tokens')}
          >
            Tokens
          </button>
          <button
            className={`nav-grid-button ${clickedButtons.has('baskets') ? 'clicked' : ''}`}
            onClick={() => handleNavButtonClick('baskets')}
          >
            Baskets
          </button>
          <button
            className={`nav-grid-button ${clickedButtons.has('exchange') ? 'clicked' : ''}`}
            onClick={() => handleNavButtonClick('exchange')}
          >
            Exchange
          </button>
        </div>

        {/* Dynamic Content Area */}
        <div className="dynamic-content-area">
          {showSendForm && (
            <div className="send-form-container">
              <TransactionForm
                onTransactionCreated={handleSendSubmit}
                balance={balance}
              />
            </div>
          )}

          {showReceiveAddress && (
            <div className="receive-address-container">
              <h3>Receive Bitcoin SV</h3>
              <p>Address copied to clipboard!</p>
              <div className="address-display">
                <code>{currentAddress || 'Generating...'}</code>
                <button
                  className={`copy-button ${copyAgainClicked ? 'clicked' : ''}`}
                  onClick={handleCopyAgain}
                >
                  {copyAgainClicked ? '✓ Copied!' : 'Copy Again'}
                </button>
              </div>
              <button
                className="close-button"
                onClick={() => setShowReceiveAddress(false)}
              >
                Close
              </button>
            </div>
          )}


          {/* Success/Error Modal */}
          {transactionResult && (
            <div className={transactionResult.success === false || transactionResult.status === 'failed' ? 'error-message' : 'success-message'}>
              {transactionResult.success === false || transactionResult.status === 'failed' ? (
                <>
                  <h3>❌ Transaction Failed</h3>
                  <div className="transaction-details">
                    <p><strong>Error:</strong> {transactionResult.error || transactionResult.message || 'Transaction broadcast failed'}</p>
                    {transactionResult.txid && (
                      <p><strong>TxID:</strong> {transactionResult.txid}</p>
                    )}
                  </div>
                  <button onClick={() => setTransactionResult(null)} className="close-button">
                    Close
                  </button>
                </>
              ) : (
                <>
                  <h3>✅ Transaction Sent!</h3>
                  <div className="transaction-details">
                    {transactionResult.txid && (
                      <p><strong>TxID:</strong> {transactionResult.txid}</p>
                    )}
                    {transactionResult.message && (
                      <p><strong>Status:</strong> {transactionResult.message}</p>
                    )}
                  </div>
                  {transactionResult.whatsOnChainUrl && (
                    <div className="whatsonchain-container">
                      <a
                        href={transactionResult.whatsOnChainUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="whatsonchain-link"
                        style={{ color: 'var(--wallet-text-light)', textDecoration: 'underline' }}
                      >
                        View on WhatsOnChain
                      </a>
                      <button
                        onClick={handleCopyLink}
                        className="copy-link-button"
                        style={{
                          marginLeft: '10px',
                          padding: '4px 8px',
                          backgroundColor: copyLinkClicked ? 'var(--wallet-dark-green)' : 'var(--wallet-gold-accent)',
                          color: copyLinkClicked ? 'var(--wallet-text-light)' : 'var(--wallet-text-dark)',
                          border: '1px solid var(--wallet-text-light)',
                          borderRadius: '4px',
                          cursor: 'pointer',
                          fontSize: '12px',
                          transition: 'all 0.2s ease'
                        }}
                      >
                        {copyLinkClicked ? '✓ Copied!' : 'Copy Link'}
                      </button>
                    </div>
                  )}
                  <button onClick={() => setTransactionResult(null)} className="close-button">
                    Close
                  </button>
                </>
              )}
            </div>
          )}

          {!showSendForm && !showReceiveAddress && !transactionResult && (
            <div className="content-placeholder">
              {addressCopiedMessage ? (
                <div className="address-copied-message">
                  ✅ {addressCopiedMessage}
                </div>
              ) : (
                "Area to render stuff the user clicks on or something"
              )}
            </div>
          )}
        </div>
    </div>
  );
};

export default WalletPanel;
