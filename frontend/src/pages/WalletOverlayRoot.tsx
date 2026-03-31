import React, { useState, useEffect, useCallback, lazy, Suspense } from 'react';
import WalletSidebar from '../components/wallet/WalletSidebar';
import DashboardTab from '../components/wallet/DashboardTab';
import '../components/wallet/WalletDashboard.css';
import '../components/TransactionComponents.css';
import '../components/WalletPanel.css';

// Lazy-load heavier tabs
const ActivityTab = lazy(() => import('../components/wallet/ActivityTab'));
const CertificatesTab = lazy(() => import('../components/wallet/CertificatesTab'));
const TokensTab = lazy(() => import('../components/wallet/TokensTab'));
const ApprovedSitesTab = lazy(() => import('../components/wallet/ApprovedSitesTab'));
const SettingsTab = lazy(() => import('../components/wallet/SettingsTab'));

const TAB_TITLES = ['Dashboard', 'Activity', 'Certificates', 'Tokens', 'Approved Sites', 'Settings'];

const WalletOverlayRoot: React.FC = () => {
  const getInitialTab = () => {
    const params = new URLSearchParams(window.location.search);
    const tab = parseInt(params.get('tab') || '0', 10);
    return tab >= 0 && tab <= 5 ? tab : 0;
  };

  const [activeTab, setActiveTab] = useState(getInitialTab);

  useEffect(() => {
    document.title = 'Hodos Wallet';
    document.body.style.margin = '0';
    document.body.style.overflow = 'hidden';
    document.body.style.background = '#0a0a0b';
  }, []);

  const handleTabChange = useCallback((tabId: number) => {
    setActiveTab(tabId);
  }, []);


  const handleRefresh = () => {
    // Force re-mount the current tab by toggling a key
    setRefreshKey((k) => k + 1);
  };

  const [refreshKey, setRefreshKey] = useState(0);

  const renderTab = () => {
    const fallback = (
      <div className="wd-loading">
        <div className="wd-spinner" />
        <span>Loading...</span>
      </div>
    );

    switch (activeTab) {
      case 0:
        return (
          <DashboardTab
            key={`dashboard-${refreshKey}`}
            onNavigateToActivity={() => setActiveTab(1)}
          />
        );
      case 1:
        return (
          <Suspense fallback={fallback}>
            <ActivityTab key={`activity-${refreshKey}`} />
          </Suspense>
        );
      case 2:
        return (
          <Suspense fallback={fallback}>
            <CertificatesTab key={`certs-${refreshKey}`} />
          </Suspense>
        );
      case 3:
        return (
          <Suspense fallback={fallback}>
            <TokensTab key={`tokens-${refreshKey}`} />
          </Suspense>
        );
      case 4:
        return (
          <Suspense fallback={fallback}>
            <ApprovedSitesTab key={`sites-${refreshKey}`} />
          </Suspense>
        );
      case 5:
        return (
          <Suspense fallback={fallback}>
            <SettingsTab key={`settings-${refreshKey}`} />
          </Suspense>
        );
      default:
        return null;
    }
  };

  return (
    <div className="wallet-dashboard">
      <WalletSidebar activeTab={activeTab} onTabChange={handleTabChange} />

      <div className="wd-content">
        <div className="wd-content-header">
          <span className="wd-content-title">{TAB_TITLES[activeTab]}</span>
          <div className="wd-header-actions">
            <button
              className="wd-icon-button"
              onClick={handleRefresh}
              title="Refresh"
            >
              &#x21BB;
            </button>
          </div>
        </div>

        <div className="wd-content-body">
          {renderTab()}
        </div>
      </div>
    </div>
  );
};

export default WalletOverlayRoot;
