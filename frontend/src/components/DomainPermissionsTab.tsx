import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { walletFetch } from '../services/walletApi';
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
import { colors as hodosColors } from '../styles/hodosTheme';

// Phase 1.5 Step 0 — shared dark-theme overrides for the Edit and Revoke
// MUI Dialogs in this file. Default MUI Dialog renders white-on-white text
// on a dark backdrop, which made the Edit form and Revoke confirmation
// nearly unreadable (memory: project_phase15_approved_sites_modal_theme).
// Centralized here so both Dialogs stay consistent if/when the palette
// shifts again. Tokens come from `frontend/src/styles/hodosTheme.ts`.
const dialogPaperSx = {
  backgroundColor: hodosColors.bgSurface,
  color: hodosColors.textPrimary,
  border: `1px solid ${hodosColors.borderDefault}`,
  borderRadius: 2,
  backgroundImage: 'none', // override MUI's default elevation gradient
};
const dialogTitleSx = {
  color: hodosColors.goldPrimary,
  fontWeight: 600,
  fontSize: '1rem',
  paddingBottom: 1,
  borderBottom: `1px solid ${hodosColors.borderSubtle}`,
};
const dialogContentTextSx = {
  color: hodosColors.textPrimary,
};
const dialogContentMutedSx = {
  color: hodosColors.textMuted,
};

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
  // Phase 1.5 Step 1 — V17 column
  identityKeyDisclosureAllowed?: boolean;
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

  // beta.3 Phase 7d item 5 — domain substring filter. The list already sorted
  // and paged (12/page); what it had no way to do was FIND a site, and after
  // Phase 7c this panel is the entire per-site permission state, so reaching a
  // specific row stopped being a convenience.
  const [query, setQuery] = useState('');

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
      const res = await walletFetch('/domain/permissions/all');
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

  // Filter by domain substring, case-insensitive. Runs BEFORE sort so the
  // sort and the pager both see the narrowed set — see the page clamp below
  // for why that matters.
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return permissions;
    return permissions.filter(p => p.domain.toLowerCase().includes(q));
  }, [permissions, query]);

  // Sort permissions
  const sorted = useMemo(() => {
    const arr = [...filtered];
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
  }, [filtered, sortKey, sortDir]);

  // Paginate
  const totalPages = Math.max(1, Math.ceil(sorted.length / ROWS_PER_PAGE));

  // 🚨 P7d-A2 — `page` is the one index-keyed thing on this surface, and a
  // filter is exactly what breaks it. Narrowing the list does NOT change
  // `permissions.length`, so the reset effect below cannot be the only guard.
  // 📏 Measured 2026-09-08 with both guards removed: filter from page 2 and the
  // table is EMPTY and stays empty, the pager vanishes (totalPages collapses to
  // 1), and the count line above still reads "4 of 15 approved sites match" —
  // no row, no pager, no explanation. Clamping derives the page from what is
  // actually on screen, so no frame is ever wrong. The effect stays for
  // behaviour, not correctness: typing should take you to the top of the
  // results rather than to the last still-valid page.
  const safePage = Math.min(page, totalPages - 1);
  const paged = useMemo(() => {
    const start = safePage * ROWS_PER_PAGE;
    return sorted.slice(start, start + ROWS_PER_PAGE);
  }, [sorted, safePage]);

  // Reset page when sort, filter, or data changes
  useEffect(() => { setPage(0); }, [sortKey, sortDir, permissions.length, query]);

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
      const res = await walletFetch('/domain/permissions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          domain: editingDomain.domain,
          trustLevel: 'approved',
          perTxLimitCents: settings.perTxLimitCents,
          perSessionLimitCents: settings.perSessionLimitCents,
          rateLimitPerMin: settings.rateLimitPerMin,
          max_tx_per_session: settings.maxTxPerSession,
          // Phase 1.5 Step 5 — persist the Personal Info Disclosure toggle.
          identityKeyDisclosureAllowed: settings.identityKeyDisclosureAllowed,
        }),
      });
      if (!res.ok) throw new Error(`Failed to update: ${res.statusText}`);
      // Invalidate the C++ DomainPermissionCache so the V17 column change
      // takes effect on the next BRC-100 call. Without this, the cache would
      // still report the old identityKeyDisclosureAllowed value.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (window as any).cefMessage?.send('domain_permission_invalidate', [editingDomain.domain]);
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
      const domain = revokeTarget.domain;
      const res = await walletFetch(
        `/domain/permissions?domain=${encodeURIComponent(domain)}`,
        { method: 'DELETE' }
      );
      if (!res.ok) throw new Error(`Failed to revoke: ${res.statusText}`);
      // Invalidate the C++ DomainPermissionCache so the next BRC-100 / BRC-121
      // request for this domain re-reads from the wallet DB. Without this, the
      // cache would still report trustLevel='approved' and the auto-approve
      // gates would incorrectly let the request through.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (window as any).cefMessage?.send('domain_permission_invalidate', [domain]);
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
      const res = await walletFetch(
        `/domain/permissions/certificate?domain=${encodeURIComponent(domain)}&cert_type=${encodeURIComponent(certType)}`,
        { method: 'DELETE' }
      );
      if (!res.ok) throw new Error(`Failed to revoke cert fields: ${res.statusText}`);
      // Invalidate cached cert-field permissions for this domain too. The C++
      // cache may have approved-fields state that needs to be re-read from DB.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (window as any).cefMessage?.send('domain_permission_invalidate', [domain]);
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

  // Phase 1.5 Step 5 — identity-key column rendered separately (non-sortable,
  // and we don't want to add a sort key for it for now since the user is more
  // likely to scan than sort by this binary field).
  const identityKeyHeaderLabel = 'Identity Key';

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
          {/* beta.3 Phase 7d item 5 — find a site.
              ⚠️ Native <input>, not MUI TextField. 📏 Measured 2026-09-08: this
              surface currently renders in a normal TAB, not an overlay —
              WalletPanel's "Manage approved sites" does
              `tab_create → /wallet?tab=4` (WalletPanel.tsx :: handleManageSites),
              so the CEF overlay input rule does not strictly bite here today.
              Native anyway, for two reasons that do: the component it opens
              (DomainPermissionForm) uses native inputs throughout and is ALSO
              rendered inside the notification overlay on the right-click path,
              and the host is named WalletOverlayRoot — if it is ever hosted as
              an overlay again, a MUI TextField would break focus silently.
              Deliberately NOT auto-focused: this panel sits below the defaults
              form, and stealing focus on mount would move the caret out from
              under a user who came to edit a default. */}
          <Box sx={{
            display: 'flex',
            alignItems: 'center',
            gap: 1.5,
            // 📏 Measured 2026-09-08: one row is right wherever it fits, but at
            // a ~250px row width the box collapsed to 22px and the count
            // overflowed the row by 142px. Wrapping degrades to the old
            // two-line layout only when there genuinely isn't room; the box's
            // minWidth is what forces the wrap instead of an unusable box.
            flexWrap: 'wrap',
            rowGap: 1,
            // 📏 The parent contributes a flat 20px between siblings. This row
            // used to add `mb: 2` on top of it, so the gap below was 36px and
            // above only 20 — the filter read as belonging to the Default
            // Limits card above rather than to the list it controls. Moved to
            // `mt` so the extra 16px sits above: 36 above, 20 below.
            mt: 2,
            // Chromium's default placeholder is near-black at reduced opacity,
            // which is unreadable on our dark surface. Same rule the cookie and
            // history panels already use.
            '& input::placeholder': { color: '#888', opacity: 1 },
          }}>
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Filter sites by domain…"
              aria-label="Filter approved sites by domain"
              style={{
                // Left-justified and capped rather than stretched: at 1910px a
                // flexed box measured 1501px, which is the "entire width it
                // doesn't need". Shrinks below the cap on narrow panels, so the
                // box — not the count — is what gives. One number to retune.
                flex: '1 1 auto',
                maxWidth: '420px',
                minWidth: '200px',        // below this the row wraps rather than shrinking the box further
                boxSizing: 'border-box',  // else maxWidth excludes padding+border
                padding: '7px 10px',
                border: `1.5px solid ${hodosColors.borderDefault}`,
                borderRadius: '6px',
                fontSize: '13px',
                fontFamily: 'inherit',
                outline: 'none',
                background: hodosColors.bgSurface,
                color: hodosColors.textPrimary,
              }}
              onFocus={(e) => { e.target.style.borderColor = hodosColors.goldPrimary; }}
              onBlur={(e) => { e.target.style.borderColor = hodosColors.borderDefault; }}
            />
            {query && (
              <HodosButton variant="secondary" size="small" onClick={() => setQuery('')}>
                Clear
              </HodosButton>
            )}
            {/* Count shares the row, hard right — `ml: auto` eats the slack the
                capped box leaves. Deliberately NOT `nowrap`/`flexShrink: 0`:
                that pinned it to max-content, and at a 236px row a 288px string
                overflowed the panel by 52px with nowhere to go. Allowed to wrap,
                it drops to its own line first and only then wraps its text. */}
            <Typography
              variant="body2"
              color="text.secondary"
              sx={{ ml: 'auto', textAlign: 'right' }}
            >
              {query
                ? `${filtered.length} of ${permissions.length} approved site${permissions.length !== 1 ? 's' : ''} match “${query.trim()}”`
                : `${permissions.length} approved site${permissions.length !== 1 ? 's' : ''}`}
            </Typography>
          </Box>
          {filtered.length === 0 ? (
            <Paper sx={{ p: 3, textAlign: 'center' }}>
              <Typography variant="body2" color="text.secondary">
                No approved site matches “{query.trim()}”.
              </Typography>
            </Paper>
          ) : (
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
                  <TableCell>
                    <span
                      title="Whether this site can read your wallet identity key. The identity key uniquely identifies you across every BRC-100 site you visit."
                      style={{ cursor: 'help' }}
                    >
                      {identityKeyHeaderLabel}
                    </span>
                  </TableCell>
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
                      <TableCell>
                        {perm.identityKeyDisclosureAllowed ? (
                          <Chip
                            label="Allowed"
                            size="small"
                            sx={{ height: 20, fontSize: '0.7rem', bgcolor: 'rgba(166, 124, 0, 0.15)', color: '#a67c00', fontWeight: 600 }}
                          />
                        ) : (
                          <Typography variant="body2" sx={{ color: '#9ca3af', fontSize: '0.75rem' }}>
                            Prompt
                          </Typography>
                        )}
                      </TableCell>
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
                        <TableCell colSpan={8} sx={{ py: 0, borderBottom: expandedDomain === perm.id ? undefined : 'none' }}>
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
          )}

          {/* Pagination. No guard needed for the no-match case: with 0 rows
              totalPages is 1, so this whole block is already absent. */}
          {totalPages > 1 && (
            <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'flex-end', mt: 1.5, gap: 1 }}>
              <Typography variant="caption" color="text.secondary">
                {safePage * ROWS_PER_PAGE + 1}–{Math.min((safePage + 1) * ROWS_PER_PAGE, sorted.length)} of {sorted.length}
              </Typography>
              <HodosButton
                variant="icon"
                size="small"
                onClick={() => setPage(Math.max(0, safePage - 1))}
                disabled={safePage === 0}
                aria-label="Previous page"
              >
                <ChevronLeftIcon fontSize="small" />
              </HodosButton>
              <HodosButton
                variant="icon"
                size="small"
                onClick={() => setPage(Math.min(totalPages - 1, safePage + 1))}
                disabled={safePage >= totalPages - 1}
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
        PaperProps={{ sx: dialogPaperSx }}
      >
        <DialogTitle sx={dialogTitleSx}>Edit Limits</DialogTitle>
        <DialogContent>
          {editingDomain && (
            <Box sx={{ pt: 2 }}>
              <DomainPermissionForm
                domain={editingDomain.domain}
                currentSettings={{
                  perTxLimitCents: editingDomain.perTxLimitCents,
                  perSessionLimitCents: editingDomain.perSessionLimitCents,
                  rateLimitPerMin: editingDomain.rateLimitPerMin,
                  maxTxPerSession: editingDomain.maxTxPerSession,
                  // Phase 1.5 Step 5 — preserve the user's identity-key choice
                  // when the form re-opens so it shows the actual current value
                  // rather than defaulting to ON every time.
                  identityKeyDisclosureAllowed: editingDomain.identityKeyDisclosureAllowed,
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
        PaperProps={{ sx: dialogPaperSx }}
      >
        <DialogTitle sx={dialogTitleSx}>Revoke Site Access</DialogTitle>
        <DialogContent>
          <Typography sx={dialogContentTextSx}>
            Are you sure you want to revoke access for <strong style={{ color: hodosColors.goldPrimary }}>{revokeTarget?.domain}</strong>?
          </Typography>
          <Typography variant="body2" sx={{ ...dialogContentMutedSx, mt: 1 }}>
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
