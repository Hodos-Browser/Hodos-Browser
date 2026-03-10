import React, { useState, useEffect, useCallback } from 'react';
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
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  IconButton,
} from '@mui/material';
import DeleteIcon from '@mui/icons-material/Delete';
import EditIcon from '@mui/icons-material/Edit';
import DomainPermissionForm, { type DomainPermissionSettings } from './DomainPermissionForm';

interface DomainPermissionRecord {
  id: number;
  domain: string;
  trustLevel: string;
  perTxLimitCents: number;
  perSessionLimitCents: number;
  rateLimitPerMin: number;
  createdAt: number;
  updatedAt: number;
}

const DomainPermissionsTab: React.FC = () => {
  const [permissions, setPermissions] = useState<DomainPermissionRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Edit modal state
  const [editingDomain, setEditingDomain] = useState<DomainPermissionRecord | null>(null);

  // Revoke confirmation state
  const [revokeTarget, setRevokeTarget] = useState<DomainPermissionRecord | null>(null);
  const [revoking, setRevoking] = useState(false);

  const fetchPermissions = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await fetch('http://localhost:31301/domain/permissions/all');
      if (!res.ok) throw new Error(`Failed to fetch: ${res.statusText}`);
      const data = await res.json();
      setPermissions(data.permissions || []);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load permissions');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchPermissions();
  }, [fetchPermissions]);

  const handleEditSave = async (settings: DomainPermissionSettings) => {
    if (!editingDomain) return;
    try {
      const res = await fetch('http://localhost:31301/domain/permissions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          domain: editingDomain.domain,
          trustLevel: 'approved',
          perTxLimitCents: settings.perTxLimitCents,
          perSessionLimitCents: settings.perSessionLimitCents,
          rateLimitPerMin: settings.rateLimitPerMin,
        }),
      });
      if (!res.ok) throw new Error(`Failed to update: ${res.statusText}`);
      setEditingDomain(null);
      fetchPermissions();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update');
    }
  };

  const handleRevoke = async () => {
    if (!revokeTarget) return;
    try {
      setRevoking(true);
      const res = await fetch(
        `http://localhost:31301/domain/permissions?domain=${encodeURIComponent(revokeTarget.domain)}`,
        { method: 'DELETE' }
      );
      if (!res.ok) throw new Error(`Failed to revoke: ${res.statusText}`);
      setRevokeTarget(null);
      fetchPermissions();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to revoke');
    } finally {
      setRevoking(false);
    }
  };

  const formatCentsAsUsd = (cents: number) => `$${(cents / 100).toFixed(2)}`;

  const formatDate = (timestamp: number) => {
    const d = new Date(timestamp * 1000);
    return d.toLocaleDateString();
  };

  if (loading) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', mt: 4 }}>
        <CircularProgress />
      </Box>
    );
  }

  return (
    <>
      {error && (
        <Alert severity="error" sx={{ mb: 2 }}>
          {error}
        </Alert>
      )}

      {permissions.length === 0 ? (
        <Paper sx={{ p: 3, textAlign: 'center' }}>
          <Typography variant="body1" color="text.secondary">
            No sites have been approved yet.
          </Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
            When you visit a BRC-100 enabled site and approve it, it will appear here.
          </Typography>
        </Paper>
      ) : (
        <>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            {permissions.length} approved site{permissions.length !== 1 ? 's' : ''}
          </Typography>
          <TableContainer component={Paper}>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>Domain</TableCell>
                  <TableCell>Per-Tx Limit</TableCell>
                  <TableCell>Per-Session Limit</TableCell>
                  <TableCell>Rate Limit</TableCell>
                  <TableCell>Approved</TableCell>
                  <TableCell align="right">Actions</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {permissions.map((perm) => (
                  <TableRow key={perm.id}>
                    <TableCell>
                      <Typography variant="body2" sx={{ fontWeight: 500 }}>
                        {perm.domain}
                      </Typography>
                    </TableCell>
                    <TableCell>{formatCentsAsUsd(perm.perTxLimitCents)}</TableCell>
                    <TableCell>{formatCentsAsUsd(perm.perSessionLimitCents)}</TableCell>
                    <TableCell>{perm.rateLimitPerMin}/min</TableCell>
                    <TableCell>{formatDate(perm.createdAt)}</TableCell>
                    <TableCell align="right">
                      <IconButton
                        size="small"
                        onClick={() => setEditingDomain(perm)}
                        title="Edit limits"
                      >
                        <EditIcon fontSize="small" />
                      </IconButton>
                      <IconButton
                        size="small"
                        onClick={() => setRevokeTarget(perm)}
                        title="Revoke access"
                        color="error"
                      >
                        <DeleteIcon fontSize="small" />
                      </IconButton>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </>
      )}

      {/* Edit Dialog */}
      <Dialog
        open={editingDomain !== null}
        onClose={() => setEditingDomain(null)}
        maxWidth="xs"
        fullWidth
      >
        <DialogTitle>Edit Limits</DialogTitle>
        <DialogContent>
          {editingDomain && (
            <Box sx={{ pt: 1 }}>
              <DomainPermissionForm
                domain={editingDomain.domain}
                currentSettings={{
                  perTxLimitCents: editingDomain.perTxLimitCents,
                  perSessionLimitCents: editingDomain.perSessionLimitCents,
                  rateLimitPerMin: editingDomain.rateLimitPerMin,
                }}
                onSave={handleEditSave}
                onCancel={() => setEditingDomain(null)}
              />
            </Box>
          )}
        </DialogContent>
      </Dialog>

      {/* Revoke Confirmation Dialog */}
      <Dialog
        open={revokeTarget !== null}
        onClose={() => setRevokeTarget(null)}
      >
        <DialogTitle>Revoke Site Access</DialogTitle>
        <DialogContent>
          <Typography>
            Are you sure you want to revoke access for <strong>{revokeTarget?.domain}</strong>?
          </Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
            This site will need to request approval again the next time it tries to interact with your wallet.
          </Typography>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setRevokeTarget(null)} disabled={revoking}>
            Cancel
          </Button>
          <Button onClick={handleRevoke} color="error" disabled={revoking}>
            {revoking ? 'Revoking...' : 'Revoke'}
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
};

export default DomainPermissionsTab;
