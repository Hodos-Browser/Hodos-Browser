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
import { useProfiles } from '../hooks/useProfiles';
import { HodosButton } from '../components/HodosButton';

// Predefined color palette for new profiles
const PROFILE_COLORS = [
    '#a67c00', // Blue
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
        switchProfile,
        createProfile,
    } = useProfiles();

    const [showCreateForm, setShowCreateForm] = useState(false);
    const [newProfileName, setNewProfileName] = useState('');
    const [selectedColor, setSelectedColor] = useState(PROFILE_COLORS[0]);
    const [avatarImage, setAvatarImage] = useState<string | null>(null);
    const nameInputRef = useRef<HTMLInputElement>(null);
    const fileInputRef = useRef<HTMLInputElement>(null);

    // Focus the input when create form shows
    useEffect(() => {
        if (showCreateForm && nameInputRef.current) {
            // Small delay to ensure DOM is ready in CEF
            setTimeout(() => {
                nameInputRef.current?.focus();
            }, 50);
        }
    }, [showCreateForm]);

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

    const handleClose = () => {
        window.cefMessage?.send('profile_panel_hide');
    };

    const handleSwitchProfile = (profileId: string) => {
        if (profileId !== currentProfile?.id) {
            switchProfile(profileId);
            // New window opens, user can close this one
        }
        handleClose();
    };

    const handleCreateProfile = () => {
        if (newProfileName.trim()) {
            createProfile(newProfileName.trim(), selectedColor, avatarImage || undefined);
            setNewProfileName('');
            setAvatarImage(null);
            setShowCreateForm(false);
            // Profile list will refresh via IPC callback
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

    return (
        <Box sx={{
            width: '100%',
            height: '100%',
            bgcolor: '#1a1d23',
            borderRadius: '8px',
            boxShadow: '0 4px 20px rgba(0,0,0,0.3)',
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
            }}>
                <Typography variant="subtitle2" sx={{ fontWeight: 600, color: '#f0f0f0' }}>
                    Profiles
                </Typography>
                <HodosButton variant="icon" size="small" onClick={handleClose} aria-label="Close">
                    <CloseIcon sx={{ fontSize: 16 }} />
                </HodosButton>
            </Box>

            {/* Profile List */}
            <Box sx={{ flex: 1, overflow: 'auto', p: 1 }}>
                {profiles.map((profile) => (
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
                        {profile.id === currentProfile?.id && (
                            <CheckIcon sx={{ fontSize: 18, color: 'primary.main' }} />
                        )}
                    </Box>
                ))}

                <Divider sx={{ my: 1, borderColor: '#2a2d35' }} />

                {/* Add Profile Section */}
                {!showCreateForm ? (
                    <Box
                        onClick={() => setShowCreateForm(true)}
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
