import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Box, Typography, Switch } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import LockIcon from '@mui/icons-material/Lock';
import LockOpenIcon from '@mui/icons-material/LockOpen';
import ErrorOutlineIcon from '@mui/icons-material/ErrorOutline';
import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined';
import ShieldIcon from '@mui/icons-material/Shield';
import StorageIcon from '@mui/icons-material/Storage';
import AccountBalanceWalletIcon from '@mui/icons-material/AccountBalanceWallet';
import ChevronRightIcon from '@mui/icons-material/ChevronRight';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import TuneIcon from '@mui/icons-material/Tune';
import PhotoCameraIcon from '@mui/icons-material/PhotoCamera';
import MicIcon from '@mui/icons-material/Mic';
import LocationOnIcon from '@mui/icons-material/LocationOn';
import NotificationsIcon from '@mui/icons-material/Notifications';
import ContentPasteIcon from '@mui/icons-material/ContentPaste';
import RestartAltIcon from '@mui/icons-material/RestartAlt';
import { HodosButton } from '../components/HodosButton';
import { usePrivacyShield } from '../hooks/usePrivacyShield';
import { useSitePermissions, type SitePermState } from '../hooks/useSitePermissions';

// code → display label + icon. Codes match kSitePermCaps in simple_handler.cpp.
const CAP_META: Record<string, { label: string; Icon: React.ElementType }> = {
    camera: { label: 'Camera', Icon: PhotoCameraIcon },
    microphone: { label: 'Microphone', Icon: MicIcon },
    location: { label: 'Location', Icon: LocationOnIcon },
    notifications: { label: 'Notifications', Icon: NotificationsIcon },
    clipboard: { label: 'Clipboard', Icon: ContentPasteIcon },
};

// Connection state mirrors MainBrowserView's securityState derivation.
type SecurityState = 'secure' | 'insecure' | 'error' | 'none';

const CONNECTION: Record<SecurityState, {
    label: string; sub: string; color: string; Icon: React.ElementType;
}> = {
    secure: {
        label: 'Connection is secure',
        sub: 'Your data is private between you and this site.',
        color: '#34a853',
        Icon: LockIcon,
    },
    insecure: {
        label: 'Not secure',
        sub: "This site doesn't use a private connection.",
        color: '#9ca3af',
        Icon: LockOpenIcon,
    },
    error: {
        label: 'Certificate problem',
        sub: "There's an issue with this site's certificate.",
        color: '#d93025',
        Icon: ErrorOutlineIcon,
    },
    none: {
        label: 'Hodos page',
        sub: '',
        color: '#9ca3af',
        Icon: InfoOutlinedIcon,
    },
};

const SiteInfoOverlayRoot: React.FC = () => {
    // Current page context, injected by C++ on each show via setSiteInfoContext.
    const [host, setHost] = useState('');
    const [security, setSecurity] = useState<SecurityState>('none');
    // Bumped on every show so usePrivacyShield re-fetches per-site state even when
    // the host is unchanged (keep-alive overlay would otherwise show stale toggles).
    const [showCount, setShowCount] = useState(0);

    const shield = usePrivacyShield(host, showCount);
    const perms = useSitePermissions(host, showCount);
    // Permissions are collapsed by default so the hub fits without scrolling; the
    // rows (which run taller than the panel) only appear when the user expands.
    const [permsOpen, setPermsOpen] = useState(false);
    const customizedCount = perms.permissions.filter((p) => p.state !== 'ask').length;

    // Auto-size the native overlay HWND to the React content height: measure after
    // each height-affecting change and ask C++ to resize (siteinfo_panel_resize).
    // The root is height:auto, so the HWND resize can't feed back into this measure.
    const rootRef = useRef<HTMLDivElement>(null);
    useLayoutEffect(() => {
        const el = rootRef.current;
        if (!el) return;
        const h = Math.ceil(el.getBoundingClientRect().height);
        if (h > 0) window.cefMessage?.send('siteinfo_panel_resize', [String(h)]);
    }, [host, security, permsOpen, perms.permissions, customizedCount, shield.masterEnabled, showCount]);

    // Register the C++ injection hook (mirrors PrivacyShield / bookmarks pattern).
    useEffect(() => {
        (window as any).setSiteInfoContext = (h: string, sec: string) => {
            setHost(h || '');
            const s = (sec || 'none') as SecurityState;
            setSecurity(['secure', 'insecure', 'error', 'none'].includes(s) ? s : 'none');
            setShowCount(c => c + 1);
        };
        return () => { delete (window as any).setSiteInfoContext; };
    }, []);

    const conn = CONNECTION[security];
    const ConnIcon = conn.Icon;
    const hasSite = !!host;

    const close = () => window.cefMessage?.send('siteinfo_panel_hide');

    const openSiteData = () => {
        if (!host) return;
        // Open in a NEW tab (matches the History menu action), not the active tab.
        // The ?domain= filter is consumed in b3; harmless until then.
        window.cefMessage?.send('tab_create', [
            `http://127.0.0.1:5137/browser-data?domain=${encodeURIComponent(host)}`,
        ]);
        close();
    };

    const openWalletPermissions = () => {
        if (!host) return;
        window.cefMessage?.send('open_wallet_permissions', [host]);
        // C++ hides the hub before opening the modal.
    };

    return (
        <Box ref={rootRef} sx={{
            width: '100%',
            // height:auto so the box wraps its content — C++ sizes the HWND to match
            // (siteinfo_panel_resize). Must NOT be height:100% or the measure loops.
            height: 'auto',
            bgcolor: '#1a1d23',
            borderRadius: '8px',
            boxShadow: '0 4px 20px rgba(0,0,0,0.15)',
            overflow: 'hidden',
            display: 'flex',
            flexDirection: 'column',
        }}>
            {/* Header */}
            <Box sx={{
                p: 1.5,
                borderBottom: '1px solid #2a2d35',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                gap: 1,
            }}>
                <Typography variant="subtitle2" sx={{
                    fontWeight: 600, color: '#f0f0f0',
                    overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                }} title={host || 'Site controls'}>
                    {host || 'Site controls'}
                </Typography>
                <HodosButton variant="icon" size="small" onClick={close} aria-label="Close">
                    <CloseIcon sx={{ fontSize: 16 }} />
                </HodosButton>
            </Box>

            {/* Natural height — C++ auto-sizes the HWND to fit this content
                (siteinfo_panel_resize), so no internal scroll/maxHeight is needed.
                A vh-based maxHeight here would be circular (vh == the HWND we size to
                the content) → a permanent scrollbar + shrink loop. */}
            <Box>
                {/* (1) Connection status */}
                <Box sx={{ p: 1.5, borderBottom: '1px solid #2a2d35', display: 'flex', gap: 1.25, alignItems: 'flex-start' }}>
                    <ConnIcon sx={{ fontSize: 20, color: conn.color, mt: '2px', flexShrink: 0 }} />
                    <Box sx={{ minWidth: 0 }}>
                        <Typography variant="body2" sx={{ fontWeight: 500, color: '#f0f0f0' }}>
                            {conn.label}
                        </Typography>
                        {conn.sub && (
                            <Typography variant="caption" sx={{ color: '#9ca3af', display: 'block' }}>
                                {conn.sub}
                            </Typography>
                        )}
                    </Box>
                </Box>

                {/* (2) Site permissions — collapsible summary row; rows indented when open */}
                {hasSite && perms.permissions.length > 0 && (
                    <Box sx={{ borderBottom: '1px solid #2a2d35' }}>
                        <Box
                            onClick={() => setPermsOpen((o) => !o)}
                            sx={{
                                p: 1.5, display: 'flex', alignItems: 'center', gap: 1.25,
                                cursor: 'pointer',
                                '&:hover': { backgroundColor: 'rgba(255,255,255,0.04)' },
                            }}
                        >
                            <TuneIcon sx={{ fontSize: 20, color: '#9ca3af', flexShrink: 0 }} />
                            <Box sx={{ flex: 1, minWidth: 0 }}>
                                <Typography variant="body2" sx={{ fontWeight: 500, color: '#f0f0f0' }}>
                                    Site permissions
                                </Typography>
                                <Typography variant="caption" sx={{ color: '#9ca3af', display: 'block' }}>
                                    {customizedCount > 0
                                        ? `${customizedCount} changed from default`
                                        : 'Camera, mic, location, notifications…'}
                                </Typography>
                            </Box>
                            <ExpandMoreIcon sx={{
                                fontSize: 20, color: '#6b7280', flexShrink: 0,
                                transform: permsOpen ? 'rotate(180deg)' : 'none',
                                transition: 'transform 0.15s',
                            }} />
                        </Box>

                        {permsOpen && (
                            <Box sx={{ pb: 0.5 }}>
                                {perms.permissions.map((p) => {
                                    const meta = CAP_META[p.code];
                                    if (!meta) return null;
                                    const MetaIcon = meta.Icon;
                                    return (
                                        <Box key={p.code} sx={{ pl: 3.5, pr: 1.5, py: 0.6, display: 'flex', alignItems: 'center', gap: 1.25 }}>
                                            <MetaIcon sx={{ fontSize: 18, color: '#9ca3af', flexShrink: 0 }} />
                                            <Typography variant="body2" sx={{ flex: 1, minWidth: 0, color: '#f0f0f0' }}>
                                                {meta.label}
                                            </Typography>
                                            <Segmented value={p.state} onChange={(s) => perms.setPermission(p.code, s)} />
                                        </Box>
                                    );
                                })}
                                <Box
                                    onClick={() => perms.resetPermissions()}
                                    sx={{
                                        pl: 3.5, pr: 1.5, py: 0.6, mt: 0.25, display: 'flex', alignItems: 'center', gap: 1,
                                        cursor: 'pointer', color: '#9ca3af',
                                        '&:hover': { backgroundColor: 'rgba(255,255,255,0.04)', color: '#f0f0f0' },
                                    }}
                                >
                                    <RestartAltIcon sx={{ fontSize: 16, flexShrink: 0 }} />
                                    <Typography variant="caption">Reset permissions for this site</Typography>
                                </Box>
                            </Box>
                        )}
                    </Box>
                )}

                {/* (3) Shields quick-toggle — delegates to the existing per-site blocking */}
                {hasSite && (
                    <Box sx={{
                        p: 1.5, borderBottom: '1px solid #2a2d35',
                        display: 'flex', alignItems: 'center', gap: 1.25,
                    }}>
                        <ShieldIcon sx={{ fontSize: 20, color: '#dfbd69', flexShrink: 0 }} />
                        <Box sx={{ flex: 1, minWidth: 0 }}>
                            <Typography variant="body2" sx={{ fontWeight: 500, color: '#f0f0f0' }}>
                                Shields for this site
                            </Typography>
                            <Typography variant="caption" sx={{ color: '#9ca3af', display: 'block' }}>
                                {shield.masterEnabled ? 'Blocking ads & trackers' : 'Blocking is off'}
                            </Typography>
                        </Box>
                        <Switch
                            size="small"
                            checked={shield.masterEnabled}
                            onChange={(e) => shield.toggleMaster(host, e.target.checked)}
                            sx={{
                                '& .MuiSwitch-switchBase.Mui-checked': { color: '#dfbd69' },
                                '& .MuiSwitch-switchBase.Mui-checked + .MuiSwitch-track': { backgroundColor: '#dfbd69' },
                            }}
                        />
                    </Box>
                )}

                {/* (4) This site's data → /browser-data (filter wiring lands in b3) */}
                {hasSite && (
                    <NavRow Icon={StorageIcon} label="This site's data" sub="View cookies & history" onClick={openSiteData} />
                )}

                {/* (5) Manage Wallet Permissions → existing edit_permissions overlay */}
                {hasSite && (
                    <NavRow Icon={AccountBalanceWalletIcon} label="Manage Wallet Permissions" sub="Spending limits & approvals" onClick={openWalletPermissions} />
                )}

                {!hasSite && (
                    <Typography variant="body2" sx={{ textAlign: 'center', py: 3, color: '#6b7280' }}>
                        No site loaded
                    </Typography>
                )}
            </Box>
        </Box>
    );
};

// Inline 3-state segmented control (Allow | Block | Ask). Avoids native <select>,
// which doesn't paint in OSR overlays (OnPopupShow/OnPopupSize are stubs).
const SEG_STATES: { key: SitePermState; label: string; activeBg: string }[] = [
    { key: 'allow', label: 'Allow', activeBg: '#34a853' },
    { key: 'block', label: 'Block', activeBg: '#d93025' },
    { key: 'ask', label: 'Ask', activeBg: '#5f6368' },
];

const Segmented: React.FC<{ value: SitePermState; onChange: (s: SitePermState) => void }> = ({ value, onChange }) => (
    <Box sx={{ display: 'flex', borderRadius: '6px', overflow: 'hidden', border: '1px solid #2a2d35', flexShrink: 0 }}>
        {SEG_STATES.map((s, i) => {
            const active = value === s.key;
            return (
                <Box
                    key={s.key}
                    onClick={() => { if (!active) onChange(s.key); }}
                    sx={{
                        px: 1, py: 0.25,
                        fontSize: '0.72rem', fontWeight: 600,
                        cursor: active ? 'default' : 'pointer',
                        color: active ? '#fff' : '#9ca3af',
                        backgroundColor: active ? s.activeBg : 'transparent',
                        borderLeft: i === 0 ? 'none' : '1px solid #2a2d35',
                        userSelect: 'none',
                        '&:hover': { backgroundColor: active ? s.activeBg : 'rgba(255,255,255,0.06)' },
                    }}
                >
                    {s.label}
                </Box>
            );
        })}
    </Box>
);

const NavRow: React.FC<{
    Icon: React.ElementType; label: string; sub: string; onClick: () => void;
}> = ({ Icon, label, sub, onClick }) => (
    <Box
        onClick={onClick}
        sx={{
            p: 1.5, borderBottom: '1px solid #2a2d35',
            display: 'flex', alignItems: 'center', gap: 1.25,
            cursor: 'pointer',
            '&:hover': { backgroundColor: 'rgba(255,255,255,0.04)' },
            '&:last-child': { borderBottom: 'none' },
        }}
    >
        <Icon sx={{ fontSize: 20, color: '#9ca3af', flexShrink: 0 }} />
        <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography variant="body2" sx={{ fontWeight: 500, color: '#f0f0f0' }}>{label}</Typography>
            <Typography variant="caption" sx={{ color: '#9ca3af', display: 'block' }}>{sub}</Typography>
        </Box>
        <ChevronRightIcon sx={{ fontSize: 18, color: '#6b7280', flexShrink: 0 }} />
    </Box>
);

export default SiteInfoOverlayRoot;
