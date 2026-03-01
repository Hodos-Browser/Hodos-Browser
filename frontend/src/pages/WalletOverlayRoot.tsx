import React, { useState, useEffect } from 'react';
import {
  Box,
  Typography,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  Button,
  CircularProgress,
  Alert,
  AppBar,
  Toolbar,
  IconButton,
  Tabs,
  Tab,
  Chip
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import RefreshIcon from '@mui/icons-material/Refresh';
import DomainPermissionsTab from '../components/DomainPermissionsTab';

interface Certificate {
  type: string;
  serial_number: string;
  subject: string;
  certifier: string;
  revocation_outpoint: string;
  signature: string;
  fields: Record<string, string>;
  keyring: Record<string, string>;
}

interface Action {
  txid: string;
  referenceNumber: string;
  status: string;
  isOutgoing: boolean;
  satoshis: number;
  description?: string;
  labels?: string[];
  version?: number;
  lockTime?: number;
  inputs?: any[];
  outputs?: any[];
}

interface Address {
  address: string;
  index: number;
  publicKey: string;
  used: boolean;
  balance?: number;
  createdAt?: number;
}

interface Output {
  txid: string;
  vout: number;
  satoshis: number;
  lockingScript: string;
  spendable: boolean;
  customInstructions?: string;
  tags?: string[];
}

interface TabPanelProps {
  children?: React.ReactNode;
  index: number;
  value: number;
}

function TabPanel(props: TabPanelProps) {
  const { children, value, index, ...other } = props;

  return (
    <div
      role="tabpanel"
      hidden={value !== index}
      id={`wallet-tabpanel-${index}`}
      aria-labelledby={`wallet-tab-${index}`}
      {...other}
      style={{ height: '100%' }}
    >
      {value === index && (
        <Box sx={{ p: 3, height: '100%' }}>
          {children}
        </Box>
      )}
    </div>
  );
}

const WalletOverlayRoot: React.FC = () => {
  const getInitialTab = () => {
    const params = new URLSearchParams(window.location.search);
    const tab = parseInt(params.get('tab') || '0', 10);
    return tab >= 0 && tab <= 4 ? tab : 0;
  };
  const [tabValue, setTabValue] = useState(getInitialTab);

  useEffect(() => {
    document.title = 'Hodos Wallet';
    document.body.style.margin = '0';
    document.body.style.overflow = 'hidden';
  }, []);

  // Export backup state
  const [showExportForm, setShowExportForm] = useState(false);
  const [exportPassword, setExportPassword] = useState('');
  const [exportConfirm, setExportConfirm] = useState('');
  const [exportError, setExportError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportSuccess, setExportSuccess] = useState(false);

  // Transactions state
  const [actions, setActions] = useState<Action[]>([]);
  const [actionsLoading, setActionsLoading] = useState(false);
  const [actionsError, setActionsError] = useState<string | null>(null);

  // Addresses state
  const [addresses, setAddresses] = useState<Address[]>([]);
  const [addressesLoading, setAddressesLoading] = useState(false);
  const [addressesError, setAddressesError] = useState<string | null>(null);

  // Outputs state
  const [outputs, setOutputs] = useState<Output[]>([]);
  const [outputsLoading, setOutputsLoading] = useState(false);
  const [outputsError, setOutputsError] = useState<string | null>(null);

  // Certificates state
  const [certificates, setCertificates] = useState<Certificate[]>([]);
  const [certificatesLoading, setCertificatesLoading] = useState(false);
  const [certificatesError, setCertificatesError] = useState<string | null>(null);

  useEffect(() => {
    console.log("💰 WalletOverlayRoot (Advanced Features) mounted");
    // Load initial tab data
    fetchActions();
  }, []);

  const handleTabChange = (event: React.SyntheticEvent, newValue: number) => {
    setTabValue(newValue);

    // Lazy load data when tab is selected
    switch (newValue) {
      case 0:
        if (actions.length === 0 && !actionsLoading) fetchActions();
        break;
      case 1:
        if (addresses.length === 0 && !addressesLoading) fetchAddresses();
        break;
      case 2:
        if (outputs.length === 0 && !outputsLoading) fetchOutputs();
        break;
      case 3:
        if (certificates.length === 0 && !certificatesLoading) fetchCertificates();
        break;
    }
  };

  const fetchActions = async () => {
    try {
      setActionsLoading(true);
      setActionsError(null);

      console.log('Fetching actions from Rust backend...');
      const response = await fetch('http://localhost:3301/listActions', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          limit: 50,
          offset: 0,
          includeLabels: true,
          includeInputs: false,
          includeOutputs: false
        }),
      });

      if (!response.ok) {
        throw new Error(`Failed to fetch actions: ${response.statusText}`);
      }

      const data = await response.json();
      console.log('Actions data:', data);

      setActions(data.actions || []);
    } catch (err) {
      console.error('Failed to fetch actions:', err);
      setActionsError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setActionsLoading(false);
    }
  };

  const fetchAddresses = async () => {
    try {
      setAddressesLoading(true);
      setAddressesError(null);

      console.log('Fetching addresses from Rust backend...');
      const response = await fetch('http://localhost:3301/wallet/addresses', {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      });

      if (!response.ok) {
        throw new Error(`Failed to fetch addresses: ${response.statusText}`);
      }

      const data = await response.json();
      console.log('Addresses data:', data);

      // Response is an array directly, not { addresses: [...] }
      setAddresses(Array.isArray(data) ? data : []);
    } catch (err) {
      console.error('Failed to fetch addresses:', err);
      setAddressesError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setAddressesLoading(false);
    }
  };

  const fetchOutputs = async () => {
    try {
      setOutputsLoading(true);
      setOutputsError(null);

      console.log('Fetching UTXOs directly from database...');

      // First get all addresses
      const addressesResp = await fetch('http://localhost:3301/wallet/addresses', {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      });

      if (!addressesResp.ok) {
        throw new Error(`Failed to fetch addresses: ${addressesResp.statusText}`);
      }

      const addressesData = await addressesResp.json();
      const addresses = addressesData.addresses || [];

      console.log('Found', addresses.length, 'addresses');

      // For now, query each address's UTXOs individually
      // TODO: Add a proper endpoint to get all UTXOs for a wallet
      const allOutputs: Output[] = [];

      // Since we don't have a direct endpoint, we'll need to use a workaround
      // The balance is calculated from UTXOs, so they exist in the database
      // but listOutputs requires a basket which our UTXOs don't have
      // For now, show a message that this feature requires basket support

      // UTXOs exist in database (balance is calculated from them)
      // but they're not assigned to baskets during balance sync
      // The /listOutputs endpoint requires a basket name
      setOutputsError('Note: UTXOs are tracked in the database for balance calculation, but are not yet assigned to baskets. Basket support for balance-synced UTXOs will be added in a future update.');
      setOutputs([]);
    } catch (err) {
      console.error('Failed to fetch outputs:', err);
      setOutputsError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setOutputsLoading(false);
    }
  };

  const fetchCertificates = async () => {
    try {
      setCertificatesLoading(true);
      setCertificatesError(null);

      console.log('Fetching certificates from Rust backend...');
      const response = await fetch('http://localhost:3301/listCertificates', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          limit: 100,
          offset: 0
        }),
      });

      if (!response.ok) {
        throw new Error(`Failed to fetch certificates: ${response.statusText}`);
      }

      const data = await response.json();
      console.log('Certificates data:', data);

      setCertificates(data.certificates || []);
    } catch (err) {
      console.error('Failed to fetch certificates:', err);
      setCertificatesError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setCertificatesLoading(false);
    }
  };

  const handleClose = () => {
    console.log("💰 Advanced features closing");
    window.close();
  };

  const handleRefresh = () => {
    switch (tabValue) {
      case 0:
        fetchActions();
        break;
      case 1:
        fetchAddresses();
        break;
      case 2:
        fetchOutputs();
        break;
      case 3:
        fetchCertificates();
        break;
    }
  };

  const handleExportBackup = async () => {
    if (exportPassword.length < 8) {
      setExportError('Password must be at least 8 characters');
      return;
    }
    if (exportPassword !== exportConfirm) {
      setExportError('Passwords do not match');
      return;
    }
    setExporting(true);
    setExportError(null);
    try {
      const res = await fetch('http://localhost:3301/wallet/export', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ password: exportPassword }),
      });
      const data = await res.json();
      if (!res.ok) {
        setExportError(data.error || 'Export failed');
        setExporting(false);
        return;
      }
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      const date = new Date().toISOString().slice(0, 10);
      a.href = url;
      a.download = `hodos-wallet-backup-${date}.hodos-wallet`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      setExportSuccess(true);
      setExportPassword('');
      setExportConfirm('');
      setTimeout(() => {
        setExportSuccess(false);
        setShowExportForm(false);
      }, 3000);
    } catch {
      setExportError('Failed to connect to wallet server');
    }
    setExporting(false);
  };

  const formatSatoshis = (sats: number): string => {
    return (sats / 100000000).toFixed(8) + ' BSV';
  };

  const truncateHash = (hash: string, start = 8, end = 8): string => {
    if (!hash || hash.length <= start + end) return hash;
    return `${hash.substring(0, start)}...${hash.substring(hash.length - end)}`;
  };

  return (
    <Box sx={{ width: '100%', height: '100vh', display: 'flex', flexDirection: 'column' }}>
      <AppBar position="static">
        <Toolbar>
          <Typography variant="h6" component="div" sx={{ flexGrow: 1 }}>
            Advanced Wallet Features
          </Typography>
          <IconButton
            size="large"
            edge="end"
            color="inherit"
            aria-label="refresh"
            onClick={handleRefresh}
            sx={{ mr: 1 }}
          >
            <RefreshIcon />
          </IconButton>
          <IconButton
            size="large"
            edge="end"
            color="inherit"
            aria-label="close"
            onClick={handleClose}
          >
            <CloseIcon />
          </IconButton>
        </Toolbar>
      </AppBar>

      <Box sx={{ borderBottom: 1, borderColor: 'divider', display: 'flex', alignItems: 'center' }}>
        <Tabs value={tabValue} onChange={handleTabChange} aria-label="wallet tabs" sx={{ flexGrow: 1 }}>
          <Tab label="Transactions" />
          <Tab label="Addresses" />
          <Tab label="UTXOs" />
          <Tab label="Certificates" />
          <Tab label="Approved Sites" />
        </Tabs>
        <Button
          variant="outlined"
          size="small"
          onClick={() => {
            setShowExportForm(!showExportForm);
            setExportError(null);
            setExportPassword('');
            setExportConfirm('');
            setExportSuccess(false);
          }}
          sx={{ mr: 2, whiteSpace: 'nowrap' }}
        >
          {showExportForm ? 'Cancel' : 'Export Backup'}
        </Button>
      </Box>

      {/* Export Backup Form */}
      {showExportForm && (
        <Box sx={{ p: 2, backgroundColor: '#f5f5f5', borderBottom: 1, borderColor: 'divider' }}>
          {exportSuccess ? (
            <Alert severity="success">Backup downloaded successfully!</Alert>
          ) : (
            <Box sx={{ display: 'flex', gap: 2, alignItems: 'center', flexWrap: 'wrap' }}>
              <Typography variant="body2" color="text.secondary" sx={{ minWidth: '200px' }}>
                Choose a password to encrypt your backup. This is NOT your PIN.
              </Typography>
              <input
                type="password"
                placeholder="Password (min 8 chars)"
                value={exportPassword}
                onChange={e => { setExportPassword(e.target.value); setExportError(null); }}
                disabled={exporting}
                style={{ padding: '6px 10px', borderRadius: '4px', border: '1px solid #ccc', fontSize: '13px' }}
              />
              <input
                type="password"
                placeholder="Confirm password"
                value={exportConfirm}
                onChange={e => { setExportConfirm(e.target.value); setExportError(null); }}
                disabled={exporting}
                style={{ padding: '6px 10px', borderRadius: '4px', border: '1px solid #ccc', fontSize: '13px' }}
              />
              <Button
                variant="contained"
                size="small"
                onClick={handleExportBackup}
                disabled={exporting || exportPassword.length < 8}
              >
                {exporting ? 'Encrypting...' : 'Download'}
              </Button>
              {exportError && (
                <Typography variant="body2" color="error" sx={{ width: '100%' }}>
                  {exportError}
                </Typography>
              )}
            </Box>
          )}
        </Box>
      )}

      <Box
        sx={{
          flex: 1,
          backgroundColor: '#f5f5f5',
          overflow: 'auto'
        }}
      >
        {/* Transactions Tab */}
        <TabPanel value={tabValue} index={0}>
          {actionsError && (
            <Alert severity="error" sx={{ mb: 2 }}>
              {actionsError}
            </Alert>
          )}

          {actionsLoading ? (
            <Box sx={{ display: 'flex', justifyContent: 'center', mt: 4 }}>
              <CircularProgress />
            </Box>
          ) : actions.length === 0 ? (
            <Paper sx={{ p: 3, textAlign: 'center' }}>
              <Typography variant="body1" color="text.secondary" gutterBottom>
                No sent transactions found.
              </Typography>
              <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
                Note: This tab shows transactions sent using the BRC-100 protocol.
                Funds received from external sources may not appear here yet.
              </Typography>
            </Paper>
          ) : (
            <>
              <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
                Total transactions: {actions.length}
              </Typography>
              <TableContainer component={Paper}>
                <Table>
                  <TableHead>
                    <TableRow>
                      <TableCell>TxID</TableCell>
                      <TableCell>Type</TableCell>
                      <TableCell>Status</TableCell>
                      <TableCell>Amount</TableCell>
                      <TableCell>Labels</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {actions.map((action, index) => (
                      <TableRow key={index}>
                        <TableCell>
                          <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.75rem' }}>
                            {truncateHash(action.txid)}
                          </Typography>
                        </TableCell>
                        <TableCell>
                          <Chip
                            label={action.isOutgoing ? 'Outgoing' : 'Incoming'}
                            color={action.isOutgoing ? 'warning' : 'success'}
                            size="small"
                          />
                        </TableCell>
                        <TableCell>
                          <Chip
                            label={action.status}
                            color={action.status === 'completed' ? 'success' : 'default'}
                            size="small"
                          />
                        </TableCell>
                        <TableCell>{formatSatoshis(action.satoshis)}</TableCell>
                        <TableCell>
                          {action.labels && action.labels.length > 0 ? (
                            <Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap' }}>
                              {action.labels.map((label, i) => (
                                <Chip key={i} label={label} size="small" variant="outlined" />
                              ))}
                            </Box>
                          ) : (
                            <Typography variant="body2" color="text.secondary">None</Typography>
                          )}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableContainer>
            </>
          )}
        </TabPanel>

        {/* Addresses Tab */}
        <TabPanel value={tabValue} index={1}>
          {addressesError && (
            <Alert severity="error" sx={{ mb: 2 }}>
              {addressesError}
            </Alert>
          )}

          {addressesLoading ? (
            <Box sx={{ display: 'flex', justifyContent: 'center', mt: 4 }}>
              <CircularProgress />
            </Box>
          ) : addresses.length === 0 ? (
            <Paper sx={{ p: 3, textAlign: 'center' }}>
              <Typography variant="body1" color="text.secondary">
                No addresses found. Generate an address to get started.
              </Typography>
            </Paper>
          ) : (
            <>
              <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
                Total addresses: {addresses.length}
              </Typography>
              <TableContainer component={Paper}>
                <Table>
                  <TableHead>
                    <TableRow>
                      <TableCell>Address</TableCell>
                      <TableCell>Index</TableCell>
                      <TableCell>Status</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {addresses.map((addr, idx) => (
                      <TableRow key={idx}>
                        <TableCell>
                          <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.85rem' }}>
                            {addr.address}
                          </Typography>
                        </TableCell>
                        <TableCell>{addr.index}</TableCell>
                        <TableCell>
                          <Chip
                            label={addr.used ? 'Used' : 'Unused'}
                            color={addr.used ? 'default' : 'primary'}
                            size="small"
                          />
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableContainer>
            </>
          )}
        </TabPanel>

        {/* UTXOs Tab */}
        <TabPanel value={tabValue} index={2}>
          {outputsError && (
            <Alert severity="error" sx={{ mb: 2 }}>
              {outputsError}
            </Alert>
          )}

          {outputsLoading ? (
            <Box sx={{ display: 'flex', justifyContent: 'center', mt: 4 }}>
              <CircularProgress />
            </Box>
          ) : outputs.length === 0 ? (
            <Paper sx={{ p: 3, textAlign: 'center' }}>
              <Typography variant="body1" color="text.secondary">
                No UTXOs found. Your unspent outputs will appear here.
              </Typography>
            </Paper>
          ) : (
            <>
              <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
                Total UTXOs: {outputs.length} | Total value: {formatSatoshis(outputs.reduce((sum, o) => sum + o.satoshis, 0))}
              </Typography>
              <TableContainer component={Paper}>
                <Table>
                  <TableHead>
                    <TableRow>
                      <TableCell>TxID</TableCell>
                      <TableCell>Vout</TableCell>
                      <TableCell>Amount</TableCell>
                      <TableCell>Spendable</TableCell>
                      <TableCell>Tags</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {outputs.map((output, index) => (
                      <TableRow key={index}>
                        <TableCell>
                          <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.75rem' }}>
                            {truncateHash(output.txid)}
                          </Typography>
                        </TableCell>
                        <TableCell>{output.vout}</TableCell>
                        <TableCell>{formatSatoshis(output.satoshis)}</TableCell>
                        <TableCell>
                          <Chip
                            label={output.spendable ? 'Yes' : 'No'}
                            color={output.spendable ? 'success' : 'default'}
                            size="small"
                          />
                        </TableCell>
                        <TableCell>
                          {output.tags && output.tags.length > 0 ? (
                            <Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap' }}>
                              {output.tags.map((tag, i) => (
                                <Chip key={i} label={tag} size="small" variant="outlined" />
                              ))}
                            </Box>
                          ) : (
                            <Typography variant="body2" color="text.secondary">None</Typography>
                          )}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableContainer>
            </>
          )}
        </TabPanel>

        {/* Certificates Tab */}
        <TabPanel value={tabValue} index={3}>
          <Box sx={{ mb: 2 }}>
            <Button
              variant="contained"
              color="primary"
              disabled
            >
              Acquire Certificate (Coming Soon)
            </Button>
          </Box>

          {certificatesError && (
            <Alert severity="error" sx={{ mb: 2 }}>
              {certificatesError}
            </Alert>
          )}

          {certificatesLoading ? (
            <Box sx={{ display: 'flex', justifyContent: 'center', mt: 4 }}>
              <CircularProgress />
            </Box>
          ) : certificates.length === 0 ? (
            <Paper sx={{ p: 3, textAlign: 'center' }}>
              <Typography variant="body1" color="text.secondary">
                No certificates found. Click "Acquire Certificate" to get your first identity certificate.
              </Typography>
            </Paper>
          ) : (
            <>
              <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
                Total certificates: {certificates.length}
              </Typography>
              <TableContainer component={Paper}>
                <Table>
                  <TableHead>
                    <TableRow>
                      <TableCell>Type</TableCell>
                      <TableCell>Certifier</TableCell>
                      <TableCell>Subject</TableCell>
                      <TableCell>Fields</TableCell>
                      <TableCell>Serial Number</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {certificates.map((cert, index) => (
                      <TableRow key={index}>
                        <TableCell>
                          {cert.type ? atob(cert.type) : 'N/A'}
                        </TableCell>
                        <TableCell>
                          <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.75rem' }}>
                            {truncateHash(cert.certifier)}
                          </Typography>
                        </TableCell>
                        <TableCell>
                          <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.75rem' }}>
                            {truncateHash(cert.subject)}
                          </Typography>
                        </TableCell>
                        <TableCell>
                          {Object.keys(cert.fields).length} field(s)
                        </TableCell>
                        <TableCell>
                          <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.75rem' }}>
                            {cert.serial_number ? truncateHash(cert.serial_number) : 'N/A'}
                          </Typography>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableContainer>
            </>
          )}
        </TabPanel>

        {/* Approved Sites Tab */}
        <TabPanel value={tabValue} index={4}>
          <DomainPermissionsTab />
        </TabPanel>
      </Box>
    </Box>
  );
};

export default WalletOverlayRoot;
