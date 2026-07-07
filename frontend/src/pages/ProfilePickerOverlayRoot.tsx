import React, { useState, useRef, useEffect } from 'react';
import {
    Box,
    Typography,
    Avatar,
    Divider,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import AddIcon from '@mui/icons-material/Add';
import CheckIcon from '@mui/icons-material/Check';
import EditIcon from '@mui/icons-material/Edit';
import StarIcon from '@mui/icons-material/Star';
import StarBorderIcon from '@mui/icons-material/StarBorder';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import ChevronLeftIcon from '@mui/icons-material/ChevronLeft';
import ChevronRightIcon from '@mui/icons-material/ChevronRight';
import { useProfiles } from '../hooks/useProfiles';
import { HodosButton } from '../components/HodosButton';
import { tokens } from '../theme/tokens';

// Predefined color palette for new profiles
const PROFILE_COLORS = [
    '#a67c00', // Gold
    '#188038', // Green
    '#a142f4', // Purple
    '#ea4335', // Red
    '#fa7b17', // Orange
    '#f538a0', // Pink
    '#00acc1', // Cyan
    '#5f6368', // Gray
];

const ProfilePickerOverlayRoot: React.FC = () => {
    const {
        profiles,
        currentProfile,
        defaultProfileId,
        switchProfile,
        createProfile,
        renameProfile,
        deleteProfile,
        setProfileColor,
        setProfileAvatar,
        setDefaultProfile,
        fetchProfiles,
    } = useProfiles();

    // Pre-window picker mode (CHUNK 2): the C++ shell loads this route as a
    // full-window chooser with `?mode=window` when launched with no profile and
    // >1 profile exists. In that mode this process owns NO profile, so EVERY
    // selection must launch a profile — the same-id short-circuit the in-session
    // dropdown uses would otherwise hang (clicking the last-used profile would
    // do nothing). C++ closes this window after the spawn.
    const isPickerWindow =
        typeof window !== 'undefined' &&
        new URLSearchParams(window.location.search).get('mode') === 'window';

    const isMac = (window as unknown as {
        hodosBrowser?: { platform?: string };
    }).hodosBrowser?.platform === 'macos';

    // Create form state
    const [showCreateForm, setShowCreateForm] = useState(false);
    const [newProfileName, setNewProfileName] = useState('');
    const [selectedColor, setSelectedColor] = useState(PROFILE_COLORS[0]);
    const [avatarImage, setAvatarImage] = useState<string | null>(null);
    const nameInputRef = useRef<HTMLInputElement>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);

    // Edit mode state
    const [editingProfileId, setEditingProfileId] = useState<string | null>(null);
    const [editName, setEditName] = useState('');
    const [editColor, setEditColor] = useState('');
    const [editAvatarImage, setEditAvatarImage] = useState<string | null>(null);
    const editNameInputRef = useRef<HTMLInputElement>(null);
    const editFileInputRef = useRef<HTMLInputElement>(null);

    // Launch-window (isPickerWindow) tile strip: horizontal scroll paged by arrows.
    const stripRef = useRef<HTMLDivElement>(null);
    const scrollStrip = (dir: number) =>
        stripRef.current?.scrollBy({ left: dir * 360, behavior: 'smooth' });

    // Reset edit/create state when overlay regains focus (re-shown after hide).
    // CEF calls SetFocus(true) on show and SetFocus(false) on hide,
    // which triggers window focus/blur events.
    useEffect(() => {
        const handleFocus = () => {
            setEditingProfileId(null);
            setShowCreateForm(false);
            setNewProfileName('');
            setAvatarImage(null);
            fetchProfiles();
        };
        window.addEventListener('focus', handleFocus);
        return () => window.removeEventListener('focus', handleFocus);
    }, [fetchProfiles]);

    // Focus the input when create form shows
    useEffect(() => {
        if (showCreateForm && nameInputRef.current) {
            setTimeout(() => {
                nameInputRef.current?.focus();
            }, 50);
        }
    }, [showCreateForm]);

    // Focus the input when edit form shows
    useEffect(() => {
        if (editingProfileId && editNameInputRef.current) {
            setTimeout(() => {
                editNameInputRef.current?.focus();
            }, 50);
        }
    }, [editingProfileId]);

    const handleImageSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (file) {
            const reader = new FileReader();
            reader.onload = (event) => {
                setAvatarImage(event.target?.result as string);
            };
            reader.readAsDataURL(file);
        }
    };

    const handleEditImageSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (file) {
            const reader = new FileReader();
            reader.onload = (event) => {
                setEditAvatarImage(event.target?.result as string);
            };
            reader.readAsDataURL(file);
        }
    };

    const handleClose = () => {
        window.cefMessage?.send('profile_panel_hide');
    };

    const handleSwitchProfile = (profileId: string) => {
        // Picker-window mode: always launch (no real "current" profile here).
        // Dropdown mode: only launch when switching to a different profile.
        if (isPickerWindow || profileId !== currentProfile?.id) {
            switchProfile(profileId);
        }
        // Dropdown closes itself; the picker window is closed by C++ after spawn.
        if (!isPickerWindow) {
            handleClose();
        }
    };

    const handleCreateProfile = () => {
        if (newProfileName.trim()) {
            createProfile(newProfileName.trim(), selectedColor, avatarImage || undefined);
            setNewProfileName('');
            setAvatarImage(null);
            setShowCreateForm(false);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter' && newProfileName.trim()) {
            handleCreateProfile();
        } else if (e.key === 'Escape') {
            setShowCreateForm(false);
            setNewProfileName('');
            setAvatarImage(null);
        }
    };

    const handleCancelCreate = () => {
        setShowCreateForm(false);
        setNewProfileName('');
        setAvatarImage(null);
    };

    // Edit handlers
    const handleEditProfile = (profile: typeof profiles[0]) => {
        setEditingProfileId(profile.id);
        setEditName(profile.name);
        setEditColor(profile.color);
        setEditAvatarImage(profile.avatarImage || null);
        setShowCreateForm(false);
    };

    const handleCancelEdit = () => {
        setEditingProfileId(null);
        setEditName('');
        setEditColor('');
        setEditAvatarImage(null);
    };

    const handleSaveEdit = () => {
        if (!editingProfileId || !editName.trim()) return;

        const profile = profiles.find(p => p.id === editingProfileId);
        if (!profile) return;

        if (editName.trim() !== profile.name) {
            renameProfile(editingProfileId, editName.trim());
        }
        if (editColor !== profile.color) {
            setProfileColor(editingProfileId, editColor);
        }
        if (editAvatarImage !== (profile.avatarImage || null)) {
            setProfileAvatar(editingProfileId, editAvatarImage || '');
        }

        setEditingProfileId(null);
        // Refresh to get consistent state from backend
        setTimeout(() => fetchProfiles(), 100);
    };

    const handleEditKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter' && editName.trim()) {
            handleSaveEdit();
        } else if (e.key === 'Escape') {
            handleCancelEdit();
        }
    };

    const handleDeleteProfile = (id: string) => {
        deleteProfile(id);
        setEditingProfileId(null);
    };

    const handleToggleDefault = (id: string) => {
        setDefaultProfile(id);
    };

    // ── Launch-window mode: a distinct, designed profile LAUNCHER (not the compact
    // in-header dropdown). Small centered window (sized by C++), gold-glow backdrop like
    // the new-tab page, logo top-left, "Choose Hodos Profile", Chrome-style tiles that
    // page left/right with arrows. The dropdown branch below is unchanged.
    if (isPickerWindow) {
        const glow = `radial-gradient(ellipse 70% 34% at 50% 60%, rgba(166, 124, 0, 0.20) 0%, transparent 62%), radial-gradient(ellipse 90% 80% at 50% 42%, rgba(255, 255, 255, 0.08) 0%, rgba(255, 255, 255, 0.02) 55%, transparent 82%), ${tokens.bgPrimary}`;
        const tileBase: React.CSSProperties = {
            position: 'relative', flex: '0 0 auto', width: 132,
            padding: '22px 10px 16px', borderRadius: 16, cursor: 'pointer',
            display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 14,
            transition: 'transform 120ms ease, box-shadow 120ms ease',
        };
        const arrowBtn: React.CSSProperties = {
            flex: '0 0 auto', width: 42, height: 42, borderRadius: '50%',
            border: `1px solid ${tokens.borderDefault}`, background: tokens.bgElevated,
            color: tokens.textSecondary, cursor: 'pointer', display: 'flex',
            alignItems: 'center', justifyContent: 'center',
        };
        return (
            <div style={{
                position: 'fixed', inset: 0, width: '100vw', height: '100vh',
                background: glow, fontFamily: tokens.fontUi, color: tokens.textPrimary,
                overflow: 'hidden', display: 'flex', flexDirection: 'column', margin: 0,
            }}>
                {/* Hide the tile-strip scrollbar — paging is via the arrow buttons. */}
                <style>{`.picker-strip{scrollbar-width:none;-ms-overflow-style:none;}.picker-strip::-webkit-scrollbar{display:none;}`}</style>
                {/* Logo — top-left; close (X) — top-right */}
                <div style={{
                    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                    padding: '18px 24px', ...(isMac ? { paddingLeft: 86, paddingTop: 12 } : {}),
                }}>
                    <img src="/Hodos_Gold_Browser_Icon.svg" alt="Hodos Browser" style={{ height: 41, width: 'auto' }} />
                    {/* No wallet/DB/adblock are started in picker mode, so this is a clean
                        exit — reuse the app's existing graceful 'exit' path (WM_CLOSE ->
                        ShutdownApplication). */}
                    <button aria-label="Close" title="Close"
                        onClick={() => window.cefMessage?.send('exit')}
                        style={{ width: 34, height: 34, borderRadius: '50%', border: 'none', background: 'transparent', color: tokens.textMuted, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center', transition: 'background 120ms, color 120ms' }}
                        onMouseEnter={(e) => { e.currentTarget.style.background = tokens.bgSurfaceHover; e.currentTarget.style.color = tokens.textPrimary; }}
                        onMouseLeave={(e) => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = tokens.textMuted; }}>
                        <CloseIcon sx={{ fontSize: 20 }} />
                    </button>
                </div>

                {/* Center content */}
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', padding: '0 20px 110px' }}>
                    {showCreateForm ? (
                        <div style={{ width: 360, maxWidth: '90%', background: tokens.bgSurface, border: `1px solid ${tokens.borderSubtle}`, borderRadius: 16, padding: 24, boxShadow: tokens.shadowLg }}>
                            <Typography variant="h6" sx={{ color: tokens.textPrimary, fontWeight: 700, mb: 2 }}>New profile</Typography>
                            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 2 }}>
                                <Avatar src={avatarImage || undefined} sx={{ width: 56, height: 56, fontSize: 22, bgcolor: selectedColor }}>
                                    {!avatarImage && (newProfileName.trim() ? newProfileName[0].toUpperCase() : '?')}
                                </Avatar>
                                <div style={{ flex: 1, background: tokens.bgElevated, border: `1px solid ${avatarImage ? tokens.gold : tokens.borderDefault}`, borderRadius: 6, padding: '6px 8px' }}>
                                    <input ref={fileInputRef} type="file" accept="image/*" onChange={handleImageSelect}
                                        style={{ fontSize: 12, color: tokens.textSecondary, width: '100%', cursor: 'pointer' }} />
                                </div>
                            </Box>
                            <input ref={nameInputRef} type="text" placeholder="Profile name" value={newProfileName}
                                onChange={(e) => setNewProfileName(e.target.value)} onKeyDown={handleKeyDown}
                                style={{ width: '100%', padding: '10px 12px', fontSize: 14, border: `1px solid ${tokens.borderDefault}`, borderRadius: 6, marginBottom: 14, boxSizing: 'border-box', outline: 'none', background: tokens.bgElevated, color: tokens.textPrimary }}
                                onFocus={(e) => e.target.style.borderColor = tokens.gold}
                                onBlur={(e) => e.target.style.borderColor = tokens.borderDefault} />
                            <Box sx={{ display: 'flex', gap: 0.75, mb: 2, flexWrap: 'wrap' }}>
                                {PROFILE_COLORS.map((color) => (
                                    <Box key={color} onClick={() => setSelectedColor(color)} sx={{ width: 26, height: 26, borderRadius: '50%', bgcolor: color, cursor: 'pointer', border: selectedColor === color ? `2px solid ${tokens.textPrimary}` : '2px solid transparent', '&:hover': { transform: 'scale(1.1)' } }} />
                                ))}
                            </Box>
                            <Box sx={{ display: 'flex', gap: 1, justifyContent: 'flex-end' }}>
                                <HodosButton variant="secondary" onClick={handleCancelCreate}>Cancel</HodosButton>
                                <HodosButton variant="primary" onClick={handleCreateProfile} disabled={!newProfileName.trim()}>Create</HodosButton>
                            </Box>
                        </div>
                    ) : editingProfileId ? (
                        <div style={{ width: 360, maxWidth: '90%', background: tokens.bgSurface, border: `1px solid ${tokens.borderSubtle}`, borderRadius: 16, padding: 24, boxShadow: tokens.shadowLg }}>
                            <Typography variant="h6" sx={{ color: tokens.textPrimary, fontWeight: 700, mb: 2 }}>Edit profile</Typography>
                            <input ref={editNameInputRef} type="text" placeholder="Profile name" value={editName}
                                onChange={(e) => setEditName(e.target.value)} onKeyDown={handleEditKeyDown}
                                style={{ width: '100%', padding: '10px 12px', fontSize: 14, border: `1px solid ${tokens.borderDefault}`, borderRadius: 6, marginBottom: 14, boxSizing: 'border-box', outline: 'none', background: tokens.bgElevated, color: tokens.textPrimary }}
                                onFocus={(e) => e.target.style.borderColor = tokens.gold}
                                onBlur={(e) => e.target.style.borderColor = tokens.borderDefault} />
                            <Box sx={{ display: 'flex', gap: 0.75, mb: 2, flexWrap: 'wrap' }}>
                                {PROFILE_COLORS.map((color) => (
                                    <Box key={color} onClick={() => setEditColor(color)} sx={{ width: 26, height: 26, borderRadius: '50%', bgcolor: color, cursor: 'pointer', border: editColor === color ? `2px solid ${tokens.textPrimary}` : '2px solid transparent', '&:hover': { transform: 'scale(1.1)' } }} />
                                ))}
                            </Box>
                            <Box onClick={() => handleToggleDefault(editingProfileId)} sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 2, cursor: 'pointer' }}>
                                {editingProfileId === defaultProfileId
                                    ? <StarIcon sx={{ fontSize: 18, color: tokens.gold }} />
                                    : <StarBorderIcon sx={{ fontSize: 18, color: tokens.textMuted }} />}
                                <Typography variant="caption" sx={{ color: tokens.textSecondary }}>
                                    {editingProfileId === defaultProfileId ? 'Default profile' : 'Set as default'}
                                </Typography>
                            </Box>
                            <Box sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
                                <HodosButton variant="secondary" onClick={handleCancelEdit}>Cancel</HodosButton>
                                <HodosButton variant="primary" onClick={handleSaveEdit} disabled={!editName.trim()}>Save</HodosButton>
                                <Box sx={{ flex: 1 }} />
                                <HodosButton variant="icon" size="small" onClick={() => handleDeleteProfile(editingProfileId)}
                                    disabled={profiles.length <= 1 || editingProfileId === defaultProfileId} aria-label="Delete profile">
                                    <DeleteOutlineIcon sx={{ fontSize: 18, color: (profiles.length <= 1 || editingProfileId === defaultProfileId) ? '#4b5563' : '#ef4444' }} />
                                </HodosButton>
                            </Box>
                        </div>
                    ) : (
                        <>
                            <h1 style={{ fontSize: 30, fontWeight: 700, margin: '0 0 40px', color: tokens.textPrimary }}>Choose Hodos Profile</h1>
                            <div style={{ display: 'flex', alignItems: 'center', gap: 12, width: '100%', maxWidth: 860, justifyContent: 'center' }}>
                                <button style={arrowBtn} onClick={() => scrollStrip(-1)} aria-label="Scroll left"><ChevronLeftIcon /></button>
                                <div ref={stripRef} className="picker-strip"
                                    style={{ display: 'flex', gap: 16, overflowX: 'auto', scrollBehavior: 'smooth', padding: '18px 8px', flex: 1, justifyContent: profiles.length <= 4 ? 'center' : 'flex-start' }}>
                                    {profiles.map((profile) => (
                                        <div key={profile.id} style={{ ...tileBase, background: `radial-gradient(ellipse 82% 60% at 50% 26%, rgba(166,124,0,0.13) 0%, transparent 72%), ${tokens.bgSurface}`, border: `1px solid rgba(166,124,0,0.45)` }}
                                            onClick={() => handleSwitchProfile(profile.id)}
                                            onMouseEnter={(e) => { e.currentTarget.style.transform = 'translateY(-4px)'; e.currentTarget.style.boxShadow = '0 10px 30px rgba(0,0,0,0.5), 0 0 22px rgba(166,124,0,0.22)'; e.currentTarget.style.borderColor = 'rgba(166,124,0,0.85)'; }}
                                            onMouseLeave={(e) => { e.currentTarget.style.transform = 'none'; e.currentTarget.style.boxShadow = 'none'; e.currentTarget.style.borderColor = 'rgba(166,124,0,0.45)'; }}>
                                            {profile.id === defaultProfileId && <StarIcon sx={{ fontSize: 15, color: tokens.gold, position: 'absolute', top: 10, left: 10 }} />}
                                            <div onClick={(e) => { e.stopPropagation(); handleEditProfile(profile); }} title="Edit profile"
                                                style={{ position: 'absolute', top: 6, right: 6, padding: 4, borderRadius: '50%', display: 'flex', opacity: 0.5 }}>
                                                <EditIcon sx={{ fontSize: 15, color: tokens.textMuted }} />
                                            </div>
                                            <Avatar src={profile.avatarImage || undefined} sx={{ width: 56, height: 56, fontSize: 22, bgcolor: profile.color }}>
                                                {!profile.avatarImage && profile.avatarInitial}
                                            </Avatar>
                                            <span style={{ fontSize: 15, fontWeight: 600, color: tokens.textPrimary, textAlign: 'center', maxWidth: '100%', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{profile.name}</span>
                                        </div>
                                    ))}
                                    <div key="__add" style={{ ...tileBase, background: tokens.bgSurface, border: `1px dashed rgba(166,124,0,0.40)` }}
                                        onClick={() => { setShowCreateForm(true); setEditingProfileId(null); }}
                                        onMouseEnter={(e) => { e.currentTarget.style.transform = 'translateY(-4px)'; }}
                                        onMouseLeave={(e) => { e.currentTarget.style.transform = 'none'; }}>
                                        <div style={{ width: 56, height: 56, borderRadius: '50%', border: `2px dashed ${tokens.textMuted}`, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                                            <AddIcon sx={{ color: tokens.textSecondary, fontSize: 26 }} />
                                        </div>
                                        <span style={{ fontSize: 15, fontWeight: 600, color: tokens.textSecondary }}>Add profile</span>
                                    </div>
                                </div>
                                <button style={arrowBtn} onClick={() => scrollStrip(1)} aria-label="Scroll right"><ChevronRightIcon /></button>
                            </div>
                        </>
                    )}
                </div>
            </div>
        );
    }

    return (
        <Box sx={{
            width: '100%',
            height: '100%',
            bgcolor: '#1a1d23',
            borderRadius: '8px',
            overflow: 'hidden',
            display: 'flex',
            flexDirection: 'column',
        }}>
            {/* Header — extra left padding in window mode on macOS to clear traffic lights */}
            <Box sx={{
                p: 1.5,
                ...(isPickerWindow && isMac ? { pl: '86px', pt: '8px' } : {}),
                borderBottom: '1px solid #2a2d35',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
            }}>
                <Typography variant="subtitle2" sx={{ fontWeight: 600, color: '#f0f0f0' }}>
                    {isPickerWindow ? 'Choose a Profile' : 'Profiles'}
                </Typography>
                {!isPickerWindow && (
                    <HodosButton variant="icon" size="small" onClick={handleClose} aria-label="Close">
                        <CloseIcon sx={{ fontSize: 16 }} />
                    </HodosButton>
                )}
            </Box>

            {/* Profile List */}
            <Box sx={{ flex: 1, overflow: 'auto', p: 1 }}>
                {profiles.map((profile) => (
                    editingProfileId === profile.id ? (
                        /* Edit Form (inline, replaces profile card) */
                        <Box key={profile.id} sx={{ p: 1, bgcolor: '#111827', borderRadius: 1, mb: 0.5 }}>
                            {/* Avatar Preview & Image Picker */}
                            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 1.5 }}>
                                <Avatar
                                    src={editAvatarImage || undefined}
                                    sx={{
                                        width: 48,
                                        height: 48,
                                        fontSize: 20,
                                        bgcolor: editColor,
                                    }}
                                >
                                    {!editAvatarImage && (editName.trim() ? editName[0].toUpperCase() : '?')}
                                </Avatar>
                                <Box sx={{ flex: 1 }}>
                                    <div style={{
                                        background: '#1a1d23',
                                        border: `1px solid ${editAvatarImage ? '#a67c00' : '#2a2d35'}`,
                                        borderRadius: '4px',
                                        padding: '6px 8px',
                                        marginBottom: editAvatarImage ? '4px' : '0',
                                    }}>
                                        <input
                                            ref={editFileInputRef}
                                            type="file"
                                            accept="image/*"
                                            onChange={handleEditImageSelect}
                                            style={{
                                                fontSize: '12px',
                                                color: '#9ca3af',
                                                width: '100%',
                                                cursor: 'pointer',
                                            }}
                                        />
                                    </div>
                                    {editAvatarImage && (
                                        <Typography
                                            variant="caption"
                                            sx={{ color: '#a67c00', cursor: 'pointer', '&:hover': { textDecoration: 'underline' } }}
                                            onClick={() => {
                                                setEditAvatarImage(null);
                                                if (editFileInputRef.current) editFileInputRef.current.value = '';
                                            }}
                                        >
                                            Remove image
                                        </Typography>
                                    )}
                                </Box>
                            </Box>

                            {/* Name Input */}
                            <input
                                ref={editNameInputRef}
                                type="text"
                                placeholder="Profile name"
                                value={editName}
                                onChange={(e) => setEditName(e.target.value)}
                                onKeyDown={handleEditKeyDown}
                                style={{
                                    width: '100%',
                                    padding: '8px 12px',
                                    fontSize: '14px',
                                    border: '1px solid #2a2d35',
                                    borderRadius: '4px',
                                    marginBottom: '12px',
                                    boxSizing: 'border-box',
                                    outline: 'none',
                                    backgroundColor: '#1a1d23',
                                    color: '#f0f0f0',
                                }}
                                onFocus={(e) => e.target.style.borderColor = '#a67c00'}
                                onBlur={(e) => e.target.style.borderColor = '#2a2d35'}
                            />

                            {/* Color Picker */}
                            <Typography variant="caption" sx={{ color: '#9ca3af', mb: 0.5, display: 'block' }}>
                                Choose color
                            </Typography>
                            <Box sx={{ display: 'flex', gap: 0.5, mb: 1.5, flexWrap: 'wrap' }}>
                                {PROFILE_COLORS.map((color) => (
                                    <Box
                                        key={color}
                                        onClick={() => setEditColor(color)}
                                        sx={{
                                            width: 24,
                                            height: 24,
                                            borderRadius: '50%',
                                            bgcolor: color,
                                            cursor: 'pointer',
                                            border: editColor === color ? '2px solid #f0f0f0' : '2px solid transparent',
                                            '&:hover': { transform: 'scale(1.1)' },
                                        }}
                                    />
                                ))}
                            </Box>

                            {/* Default Profile Toggle */}
                            <Box
                                onClick={() => handleToggleDefault(profile.id)}
                                sx={{
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: 1,
                                    mb: 1.5,
                                    p: 0.5,
                                    borderRadius: 1,
                                    cursor: 'pointer',
                                    '&:hover': { bgcolor: '#1a1d23' },
                                }}
                            >
                                {profile.id === defaultProfileId ? (
                                    <StarIcon sx={{ fontSize: 18, color: '#a67c00' }} />
                                ) : (
                                    <StarBorderIcon sx={{ fontSize: 18, color: '#6b7280' }} />
                                )}
                                <Typography variant="caption" sx={{ color: '#9ca3af' }}>
                                    {profile.id === defaultProfileId ? 'Default profile' : 'Set as default'}
                                </Typography>
                            </Box>

                            {/* Action Buttons */}
                            <Box sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
                                <HodosButton variant="secondary" onClick={handleCancelEdit}>
                                    Cancel
                                </HodosButton>
                                <HodosButton variant="primary" onClick={handleSaveEdit} disabled={!editName.trim()}>
                                    Save
                                </HodosButton>
                                <Box sx={{ flex: 1 }} />
                                <HodosButton
                                    variant="icon"
                                    size="small"
                                    onClick={() => handleDeleteProfile(profile.id)}
                                    disabled={profiles.length <= 1 || profile.id === defaultProfileId}
                                    aria-label="Delete profile"
                                >
                                    <DeleteOutlineIcon sx={{
                                        fontSize: 18,
                                        color: (profiles.length <= 1 || profile.id === defaultProfileId) ? '#4b5563' : '#ef4444',
                                    }} />
                                </HodosButton>
                            </Box>
                        </Box>
                    ) : (
                        /* Normal Profile Card */
                        <Box
                            key={profile.id}
                            onClick={() => handleSwitchProfile(profile.id)}
                            sx={{
                                display: 'flex',
                                alignItems: 'center',
                                gap: 1.5,
                                p: 1,
                                borderRadius: 1,
                                cursor: 'pointer',
                                bgcolor: profile.id === currentProfile?.id ? '#1a1a2e' : 'transparent',
                                '&:hover': {
                                    bgcolor: '#1f2937',
                                },
                                '&:hover .edit-btn': {
                                    opacity: 1,
                                },
                            }}
                        >
                            <Avatar
                                src={profile.avatarImage || undefined}
                                sx={{
                                    width: 32,
                                    height: 32,
                                    fontSize: 14,
                                    bgcolor: profile.color,
                                }}
                            >
                                {!profile.avatarImage && profile.avatarInitial}
                            </Avatar>
                            <Typography
                                variant="body2"
                                sx={{
                                    flex: 1,
                                    fontWeight: profile.id === currentProfile?.id ? 600 : 400,
                                    color: '#f0f0f0',
                                }}
                            >
                                {profile.name}
                            </Typography>
                            {profile.id === defaultProfileId && (
                                <StarIcon sx={{ fontSize: 14, color: '#a67c00' }} />
                            )}
                            {profile.id === currentProfile?.id && (
                                <CheckIcon sx={{ fontSize: 18, color: 'primary.main' }} />
                            )}
                            <Box
                                className="edit-btn"
                                onClick={(e) => {
                                    e.stopPropagation();
                                    handleEditProfile(profile);
                                }}
                                sx={{
                                    opacity: 0,
                                    p: 0.5,
                                    borderRadius: '50%',
                                    display: 'flex',
                                    alignItems: 'center',
                                    '&:hover': { bgcolor: '#374151' },
                                }}
                            >
                                <EditIcon sx={{ fontSize: 14, color: '#9ca3af' }} />
                            </Box>
                        </Box>
                    )
                ))}

                <Divider sx={{ my: 1, borderColor: '#2a2d35' }} />

                {/* Add Profile Section */}
                {!showCreateForm ? (
                    <Box
                        onClick={() => { setShowCreateForm(true); setEditingProfileId(null); }}
                        sx={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: 1.5,
                            p: 1,
                            borderRadius: 1,
                            cursor: 'pointer',
                            '&:hover': {
                                bgcolor: '#1f2937',
                            },
                        }}
                    >
                        <Avatar
                            sx={{
                                width: 32,
                                height: 32,
                                bgcolor: 'transparent',
                                border: '2px dashed #6b7280',
                            }}
                        >
                            <AddIcon sx={{ color: '#9ca3af' }} />
                        </Avatar>
                        <Typography variant="body2" sx={{ color: '#9ca3af' }}>
                            Add profile
                        </Typography>
                    </Box>
                ) : (
                    <Box sx={{ p: 1 }}>
                        {/* Avatar Preview & Image Picker */}
                        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, mb: 1.5 }}>
                            <Avatar
                                src={avatarImage || undefined}
                                sx={{
                                    width: 48,
                                    height: 48,
                                    fontSize: 20,
                                    bgcolor: selectedColor,
                                }}
                            >
                                {!avatarImage && (newProfileName.trim() ? newProfileName[0].toUpperCase() : '?')}
                            </Avatar>
                            <Box sx={{ flex: 1 }}>
                                {/* Visible file input - CEF handles these better than hidden ones */}
                                <div style={{
                                    background: '#111827',
                                    border: `1px solid ${avatarImage ? '#a67c00' : '#2a2d35'}`,
                                    borderRadius: '4px',
                                    padding: '6px 8px',
                                    marginBottom: avatarImage ? '4px' : '0',
                                }}>
                                    <input
                                        ref={fileInputRef}
                                        type="file"
                                        accept="image/*"
                                        onChange={handleImageSelect}
                                        style={{
                                            fontSize: '12px',
                                            color: '#9ca3af',
                                            width: '100%',
                                            cursor: 'pointer',
                                        }}
                                    />
                                </div>
                                {avatarImage && (
                                    <Typography
                                        variant="caption"
                                        sx={{ color: '#a67c00', cursor: 'pointer', '&:hover': { textDecoration: 'underline' } }}
                                        onClick={() => {
                                            setAvatarImage(null);
                                            if (fileInputRef.current) fileInputRef.current.value = '';
                                        }}
                                    >
                                        Remove image
                                    </Typography>
                                )}
                            </Box>
                        </Box>

                        {/* Native input for CEF focus compatibility */}
                        <input
                            ref={nameInputRef}
                            type="text"
                            placeholder="Profile name"
                            value={newProfileName}
                            onChange={(e) => setNewProfileName(e.target.value)}
                            onKeyDown={handleKeyDown}
                            style={{
                                width: '100%',
                                padding: '8px 12px',
                                fontSize: '14px',
                                border: '1px solid #2a2d35',
                                borderRadius: '4px',
                                marginBottom: '12px',
                                boxSizing: 'border-box',
                                outline: 'none',
                                backgroundColor: '#111827',
                                color: '#f0f0f0',
                            }}
                            onFocus={(e) => e.target.style.borderColor = '#a67c00'}
                            onBlur={(e) => e.target.style.borderColor = '#2a2d35'}
                        />

                        {/* Color Picker */}
                        <Typography variant="caption" sx={{ color: '#9ca3af', mb: 0.5, display: 'block' }}>
                            Choose color
                        </Typography>
                        <Box sx={{ display: 'flex', gap: 0.5, mb: 1.5, flexWrap: 'wrap' }}>
                            {PROFILE_COLORS.map((color) => (
                                <Box
                                    key={color}
                                    onClick={() => setSelectedColor(color)}
                                    sx={{
                                        width: 24,
                                        height: 24,
                                        borderRadius: '50%',
                                        bgcolor: color,
                                        cursor: 'pointer',
                                        border: selectedColor === color ? '2px solid #f0f0f0' : '2px solid transparent',
                                        '&:hover': {
                                            transform: 'scale(1.1)',
                                        },
                                    }}
                                />
                            ))}
                        </Box>

                        <Box sx={{ display: 'flex', gap: 1 }}>
                            <HodosButton variant="secondary" onClick={handleCancelCreate}>
                                Cancel
                            </HodosButton>
                            <HodosButton variant="primary" onClick={handleCreateProfile} disabled={!newProfileName.trim()}>
                                Create
                            </HodosButton>
                        </Box>
                    </Box>
                )}
            </Box>

            {/* Footer hint */}
            <Box sx={{
                p: 1,
                borderTop: '1px solid #2a2d35',
                bgcolor: '#111827',
            }}>
                <Typography variant="caption" sx={{ color: '#6b7280', display: 'block', textAlign: 'center' }}>
                    Switching profiles opens a new window
                </Typography>
            </Box>
        </Box>
    );
};

export default ProfilePickerOverlayRoot;
