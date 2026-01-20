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
  IconButton
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';

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

const WalletOverlayRoot: React.FC = () => {
  const [certificates, setCertificates] = useState<Certificate[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [totalCertificates, setTotalCertificates] = useState(0);

  useEffect(() => {
    console.log("💰 WalletOverlayRoot (Advanced Features) mounted");
    fetchCertificates();
  }, []);

  const fetchCertificates = async () => {
    try {
      setLoading(true);
      setError(null);

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
      setTotalCertificates(data.total_certificates || 0);
    } catch (err) {
      console.error('Failed to fetch certificates:', err);
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setLoading(false);
    }
  };

  const handleAcquireCertificate = () => {
    console.log('Acquire certificate clicked - feature not yet implemented');
    alert('Certificate acquisition feature coming soon!');
  };

  const handleClose = () => {
    console.log("💰 Advanced features closing");
    window.cefMessage?.send('overlay_close', []);
  };

  return (
    <Box sx={{ width: '100%', height: '100vh', display: 'flex', flexDirection: 'column' }}>
      <AppBar position="static">
        <Toolbar>
          <Typography variant="h6" component="div" sx={{ flexGrow: 1 }}>
            Advanced Features - BRC-100 Certificates
          </Typography>
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

      <Box
        sx={{
          flex: 1,
          padding: 3,
          backgroundColor: '#f5f5f5',
          overflow: 'auto'
        }}
      >
        <Box sx={{ mb: 2, display: 'flex', gap: 2, alignItems: 'center' }}>
          <Button
            variant="contained"
            color="primary"
            onClick={handleAcquireCertificate}
            disabled
          >
            Acquire Certificate (Coming Soon)
          </Button>
          <Button
            variant="outlined"
            onClick={fetchCertificates}
            disabled={loading}
          >
            Refresh
          </Button>
        </Box>

        {error && (
          <Alert severity="error" sx={{ mb: 2 }}>
            {error}
          </Alert>
        )}

        {loading ? (
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
              Total certificates: {totalCertificates}
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
                          {cert.certifier.substring(0, 16)}...
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.75rem' }}>
                          {cert.subject.substring(0, 16)}...
                        </Typography>
                      </TableCell>
                      <TableCell>
                        {Object.keys(cert.fields).length} field(s)
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.75rem' }}>
                          {cert.serial_number ? cert.serial_number.substring(0, 16) : 'N/A'}...
                        </Typography>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          </>
        )}
      </Box>
    </Box>
  );
};

export default WalletOverlayRoot;
