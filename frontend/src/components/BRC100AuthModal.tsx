import React, { useState } from 'react';
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
  Typography,
  Box,
  Card,
  CardContent,
  Checkbox,
  FormControlLabel,
  Chip,
  Avatar,
  Divider,
  Alert,
  IconButton
} from '@mui/material';
import {
  Security as SecurityIcon,
  Web as WebIcon,
  CheckCircle as CheckCircleIcon,
  Cancel as CancelIcon,
  Close as CloseIcon
} from '@mui/icons-material';

interface BRC100AuthRequest {
  domain: string;
  appId: string;
  purpose: string;
  challenge: string;
  sessionDuration?: number;
  permissions: string[];
}

interface BRC100AuthModalProps {
  open: boolean;
  onClose: () => void;
  onApprove: (whitelist: boolean) => void;
  onReject: () => void;
  request: BRC100AuthRequest;
}

const BRC100AuthModal: React.FC<BRC100AuthModalProps> = ({
  open,
  onClose,
  onApprove,
  onReject,
  request
}) => {
  const [whitelistDomain, setWhitelistDomain] = useState(true);

  const handleApprove = () => {
    onApprove(whitelistDomain);
  };

  const handleReject = () => {
    onReject();
  };

  const formatDomain = (domain: string) => {
    return domain.replace(/^https?:\/\//, '').replace(/^www\./, '');
  };

  const getDomainIcon = (domain: string) => {
    const cleanDomain = formatDomain(domain);
    return cleanDomain.charAt(0).toUpperCase();
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      maxWidth="sm"
      fullWidth
      PaperProps={{
        sx: {
          borderRadius: 2,
          boxShadow: '0 8px 32px rgba(0,0,0,0.12)',
        }
      }}
    >
      <DialogTitle sx={{ pb: 1 }}>
        <Box display="flex" alignItems="center" justifyContent="space-between">
          <Box display="flex" alignItems="center">
            <SecurityIcon color="primary" sx={{ mr: 1 }} />
            <Typography variant="h6" component="div">
              Authentication Request
            </Typography>
          </Box>
          <IconButton
            onClick={onClose}
            size="small"
            sx={{ color: 'text.secondary' }}
          >
            <CloseIcon />
          </IconButton>
        </Box>
      </DialogTitle>

      <Divider />

      <DialogContent sx={{ pt: 3 }}>
        {/* App Information */}
        <Card variant="outlined" sx={{ mb: 3 }}>
          <CardContent>
            <Box display="flex" alignItems="center" mb={2}>
              <Avatar
                sx={{
                  bgcolor: 'primary.main',
                  mr: 2,
                  width: 48,
                  height: 48,
                  fontSize: '1.2rem'
                }}
              >
                {getDomainIcon(request.domain)}
              </Avatar>
              <Box>
                <Typography variant="h6" gutterBottom>
                  {formatDomain(request.domain)}
                </Typography>
                <Box display="flex" alignItems="center">
                  <WebIcon fontSize="small" sx={{ mr: 0.5, color: 'text.secondary' }} />
                  <Typography variant="body2" color="text.secondary">
                    {request.domain}
                  </Typography>
                </Box>
              </Box>
            </Box>

            <Typography variant="body2" color="text.secondary">
              <strong>Purpose:</strong> {request.purpose}
            </Typography>

            {request.sessionDuration && (
              <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
                <strong>Session Duration:</strong> {request.sessionDuration} minutes
              </Typography>
            )}
          </CardContent>
        </Card>

        {/* Permissions */}
        <Box mb={3}>
          <Typography variant="subtitle2" gutterBottom>
            This app is requesting permission to:
          </Typography>
          <Box display="flex" flexWrap="wrap" gap={1}>
            {request.permissions.map((permission, index) => (
              <Chip
                key={index}
                label={permission}
                size="small"
                color="primary"
                variant="outlined"
                icon={<CheckCircleIcon />}
              />
            ))}
          </Box>
        </Box>

        {/* Whitelist Option */}
        <FormControlLabel
          control={
            <Checkbox
              checked={whitelistDomain}
              onChange={(e) => setWhitelistDomain(e.target.checked)}
              color="primary"
            />
          }
          label={
            <Box>
              <Typography variant="body2">
                Whitelist this site
              </Typography>
              <Typography variant="caption" color="text.secondary">
                Automatically approve future requests from this domain
              </Typography>
            </Box>
          }
        />

        {/* Security Notice */}
        <Alert severity="info" sx={{ mt: 2 }}>
          <Typography variant="caption">
            Only approve requests from trusted websites. This will allow the site to access your BRC-100 identity.
          </Typography>
        </Alert>
      </DialogContent>

      <Divider />

      <DialogActions sx={{ p: 2, gap: 1 }}>
        <Button
          onClick={handleReject}
          variant="outlined"
          color="error"
          startIcon={<CancelIcon />}
          sx={{ minWidth: 120 }}
        >
          Reject
        </Button>
        <Button
          onClick={handleApprove}
          variant="contained"
          color="primary"
          startIcon={<CheckCircleIcon />}
          sx={{ minWidth: 120 }}
        >
          Approve
        </Button>
      </DialogActions>
    </Dialog>
  );
};

export default BRC100AuthModal;
