import React, { useState, useEffect, useCallback, useMemo } from 'react';
import {
  Box,
  Typography,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TableSortLabel,
  Paper,
  CircularProgress,
  Alert,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Collapse,
  Chip,
} from '@mui/material';
import DeleteIcon from '@mui/icons-material/Delete';
import EditIcon from '@mui/icons-material/Edit';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import ExpandLessIcon from '@mui/icons-material/ExpandLess';
import ChevronLeftIcon from '@mui/icons-material/ChevronLeft';
import ChevronRightIcon from '@mui/icons-material/ChevronRight';
import DomainPermissionForm, { type DomainPermissionSettings } from './DomainPermissionForm';
import { HodosButton } from './HodosButton';

interface CertFieldPermission {
  certType: string;
  fields: string[];
}

interface DomainPermissionRecord {
  id: number;
  domain: string;
  trustLevel: string;
  perTxLimitCents: number;
  perSessionLimitCents: number;
  rateLimitPerMin: number;
  maxTxPerSession: number;
  createdAt: number;
  updatedAt: number;
  certFieldPermissions?: CertFieldPermission[];
}

type SortKey = 'domain' | 'perTxLimitCents' | 'perSessionLimitCents' | 'rateLimitPerMin' | 'maxTxPerSession' | 'createdAt';
type SortDir = 'asc' | 'desc';

const ROWS_PER_PAGE = 12;

const formatFieldName = (field: string) =>
  field.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());

const DomainPermissionsTab: React.FC = () => {
  const [permissions, setPermissions] = useState<DomainPermissionRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedDomain, setExpandedDomain] = useState<number | null>(null);

  // Sorting
  const [sortKey, setSortKey] = useState<SortKey>('domain');
  const [sortDir, setSortDir] = useState<SortDir>('asc');

  // Pagination
  const [page, setPage] = useState(0);

  // Edit modal state
  const [editingDomain, setEditingDomain] = useState<DomainPermissionRecord | null>(null);

  // Revoke confirmation state
  const [revokeTarget, setRevokeTarget] = useState<DomainPermissionRecord | null>(null);
  const [revoking, setRevoking] = useState(false);

  const fetchPermissions = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await fetch('http://127.0.0.1:31301/domain/permissions/all');
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

  // Sort permissions
  const sorted = useMemo(() => {
    const arr = [...permissions];
    arr.sort((a, b) => {
      let cmp = 0;
      if (sortKey === 'domain') {
        cmp = a.domain.localeCompare(b.domain);
      } else {
        cmp = (a[sortKey] as number) - (b[sortKey] as number);
      }
      return sortDir === 'asc' ? cmp : -cmp;
    });
    return arr;
  }, [permissions, sortKey, sortDir]);

  // Paginate
  const totalPages = Math.max(1, Math.ceil(sorted.length / ROWS_PER_PAGE));
  const paged = useMemo(() => {
    const start = page * ROWS_PER_PAGE;
    return sorted.slice(start, start + ROWS_PER_PAGE);
  }, [sorted, page]);

  // Reset page when sort changes or data changes
  useEffect(() => { setPage(0); }, [sortKey, sortDir, permissions.length]);

  const handleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDir(d => d === 'asc' ? 'desc' : 'asc');
    } else {
      setSortKey(key);
      setSortDir('asc');
    }
  };

  const handleEditSave = async (settings: DomainPermissionSettings) => {
    if (!editingDomain) return;
    try {
      const res = await fetch('http://127.0.0.1:31301/domain/permissions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          domain: editingDomain.domain,
          trustLevel: 'approved',
          perTxLimitCents: settings.perTxLimitCents,
          perSessionLimitCents: settings.perSessionLimitCents,
          rateLimitPerMin: settings.rateLimitPerMin,
          max_tx_per_session: settings.maxTxPerSession,
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
        `http://127.0.0.1:31301/domain/permissions?domain=${encodeURIComponent(revokeTarget.domain)}`,
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

  const handleRevokeCertType = async (domain: string, certType: string) => {
    try {
      const res = await fetch(
        `http://127.0.0.1:31301/domain/permissions/certificate?domain=${encodeURIComponent(domain)}&cert_type=${encodeURIComponent(certType)}`,
        { method: 'DELETE' }
      );
      if (!res.ok) throw new Error(`Failed to revoke cert fields: ${res.statusText}`);
      fetchPermissions();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to revoke cert fields');
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

  const hasCertFields = (perm: DomainPermissionRecord) =>
    perm.certFieldPermissions && perm.certFieldPermissions.length > 0;

  const columns: { key: SortKey; label: string; align?: 'right' }[] = [
    { key: 'domain', label: 'Domain' },
    { key: 'perTxLimitCents', label: 'Per-Tx Limit' },
    { key: 'perSessionLimitCents', label: 'Per-Session Limit' },
    { key: 'rateLimitPerMin', label: 'Rate Limit' },
    { key: 'maxTxPerSession', label: 'Tx/Session' },
    { key: 'createdAt', label: 'Approved' },
  ];

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
                  {columns.map((col) => (
                    <TableCell key={col.key}>
                      <TableSortLabel
                        active={sortKey === col.key}
                        direction={sortKey === col.key ? sortDir : 'asc'}
                        onClick={() => handleSort(col.key)}
                        sx={{
                          color: 'inherit !important',
                          '&.Mui-active': { color: 'inherit !important' },
                          '& .MuiTableSortLabel-icon': { color: 'inherit !important' },
                        }}
                      >
                        {col.label}
                      </TableSortLabel>
                    </TableCell>
                  ))}
                  <TableCell align="right">Actions</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {paged.map((perm) => (
                  <React.Fragment key={perm.id}>
                    <TableRow
                      sx={hasCertFields(perm) ? { cursor: 'pointer' } : undefined}
                      onClick={() => hasCertFields(perm) && setExpandedDomain(expandedDomain === perm.id ? null : perm.id)}
                    >
                      <TableCell>
                        <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
                          {hasCertFields(perm) && (
                            expandedDomain === perm.id
                              ? <ExpandLessIcon fontSize="small" sx={{ color: '#9ca3af' }} />
                              : <ExpandMoreIcon fontSize="small" sx={{ color: '#9ca3af' }} />
                          )}
                          <Typography variant="body2" sx={{ fontWeight: 500 }}>
                            {perm.domain}
                          </Typography>
                          {hasCertFields(perm) && (
                            <Chip
                              label={`${perm.certFieldPermissions!.reduce((sum, ct) => sum + ct.fields.length, 0)} cert fields`}
                              size="small"
                              sx={{ ml: 1, height: 20, fontSize: '0.7rem', bgcolor: '#2d2d2d', color: '#a67c00' }}
                            />
                          )}
                        </Box>
                      </TableCell>
                      <TableCell>{formatCentsAsUsd(perm.perTxLimitCents)}</TableCell>
                      <TableCell>{formatCentsAsUsd(perm.perSessionLimitCents)}</TableCell>
                      <TableCell>{perm.rateLimitPerMin}/min</TableCell>
                      <TableCell>{perm.maxTxPerSession}</TableCell>
                      <TableCell>{formatDate(perm.createdAt)}</TableCell>
                      <TableCell align="right">
                        <HodosButton
                          variant="icon"
                          size="small"
                          onClick={(e) => { e.stopPropagation(); setEditingDomain(perm); }}
                          aria-label="Edit limits"
                          title="Edit limits"
                        >
                          <EditIcon fontSize="small" />
                        </HodosButton>
                        <HodosButton
                          variant="icon"
                          size="small"
                          onClick={(e) => { e.stopPropagation(); setRevokeTarget(perm); }}
                          aria-label="Revoke access"
                          title="Revoke access"
                        >
                          <DeleteIcon fontSize="small" />
                        </HodosButton>
                      </TableCell>
                    </TableRow>
                    {hasCertFields(perm) && (
                      <TableRow>
                        <TableCell colSpan={7} sx={{ py: 0, borderBottom: expandedDomain === perm.id ? undefined : 'none' }}>
                          <Collapse in={expandedDomain === perm.id} timeout="auto" unmountOnExit>
                            <Box sx={{ py: 1.5, pl: 4 }}>
                              <Typography variant="caption" color="text.secondary" sx={{ fontWeight: 600, mb: 1, display: 'block' }}>
                                Certificate Field Permissions
                              </Typography>
                              {perm.certFieldPermissions!.map((ct) => (
                                <Box key={ct.certType} sx={{ mb: 1, pl: 2, display: 'flex', alignItems: 'center' }}>
                                  <Typography variant="caption" color="text.secondary" sx={{ minWidth: 50, flexShrink: 0 }}>
                                    Fields:
                                  </Typography>
                                  <Box sx={{ display: 'flex', gap: 0.5, flexWrap: 'wrap', overflow: 'hidden', maxWidth: 420, position: 'relative' }}
                                    title={ct.fields.map(formatFieldName).join(', ')}
                                  >
                                    {ct.fields.map((field) => (
                                      <Chip
                                        key={field}
                                        label={formatFieldName(field)}
                                        size="small"
                                        sx={{ height: 22, fontSize: '0.75rem', color: '#e5e7eb', bgcolor: '#374151' }}
                                      />
                                    ))}
                                  </Box>
                                  <Box sx={{ ml: 'auto', pl: 2, flexShrink: 0 }}>
                                    <HodosButton
                                      variant="icon"
                                      size="small"
                                      onClick={() => handleRevokeCertType(perm.domain, ct.certType)}
                                      aria-label="Remove cert permission"
                                      title="Remove certificate field permission"
                                    >
                                      <DeleteIcon fontSize="small" sx={{ color: '#ef4444' }} />
                                    </HodosButton>
                                  </Box>
                                </Box>
                              ))}
                            </Box>
                          </Collapse>
                        </TableCell>
                      </TableRow>
                    )}
                  </React.Fragment>
                ))}
              </TableBody>
            </Table>
          </TableContainer>

          {/* Pagination */}
          {totalPages > 1 && (
            <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'flex-end', mt: 1.5, gap: 1 }}>
              <Typography variant="caption" color="text.secondary">
                {page * ROWS_PER_PAGE + 1}–{Math.min((page + 1) * ROWS_PER_PAGE, sorted.length)} of {sorted.length}
              </Typography>
              <HodosButton
                variant="icon"
                size="small"
                onClick={() => setPage(p => Math.max(0, p - 1))}
                disabled={page === 0}
                aria-label="Previous page"
              >
                <ChevronLeftIcon fontSize="small" />
              </HodosButton>
              <HodosButton
                variant="icon"
                size="small"
                onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
                disabled={page >= totalPages - 1}
                aria-label="Next page"
              >
                <ChevronRightIcon fontSize="small" />
              </HodosButton>
            </Box>
          )}
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
                  maxTxPerSession: editingDomain.maxTxPerSession,
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
          <HodosButton variant="secondary" size="small" onClick={() => setRevokeTarget(null)} disabled={revoking}>
            Cancel
          </HodosButton>
          <HodosButton
            variant="danger"
            size="small"
            onClick={handleRevoke}
            loading={revoking}
            loadingText="Revoking..."
          >
            Revoke
          </HodosButton>
        </DialogActions>
      </Dialog>
    </>
  );
};

export default DomainPermissionsTab;
