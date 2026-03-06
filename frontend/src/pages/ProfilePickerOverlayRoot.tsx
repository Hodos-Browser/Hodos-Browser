import React, { useState, useRef, useEffect } from 'react';
import {
    Box,
    Typography,
    IconButton,
    Avatar,
    Divider,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import AddIcon from '@mui/icons-material/Add';
import CheckIcon from '@mui/icons-material/Check';
import { useProfiles } from '../hooks/useProfiles';

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
            bgcolor: '#fff',
            borderRadius: '8px',
            boxShadow: '0 4px 20px rgba(0,0,0,0.15)',
            overflow: 'hidden',
            display: 'flex',
            flexDirection: 'column',
        }}>
            {/* Header */}
            <Box sx={{
                p: 1.5,
                borderBottom: '1px solid rgba(0,0,0,0.08)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
            }}>
                <Typography variant="subtitle2" sx={{ fontWeight: 600, color: 'rgba(0,0,0,0.87)' }}>
                    Profiles
                </Typography>
                <IconButton
                    size="small"
                    onClick={handleClose}
                    sx={{ p: 0.5 }}
                >
                    <CloseIcon sx={{ fontSize: 16 }} />
                </IconButton>
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
                            bgcolor: profile.id === currentProfile?.id ? 'rgba(0,0,0,0.04)' : 'transparent',
                            '&:hover': {
                                bgcolor: 'rgba(0,0,0,0.06)',
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
                            }}
                        >
                            {profile.name}
                        </Typography>
                        {profile.id === currentProfile?.id && (
                            <CheckIcon sx={{ fontSize: 18, color: 'primary.main' }} />
                        )}
                    </Box>
                ))}

                <Divider sx={{ my: 1 }} />

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
                                bgcolor: 'rgba(0,0,0,0.06)',
                            },
                        }}
                    >
                        <Avatar
                            sx={{
                                width: 32,
                                height: 32,
                                bgcolor: 'transparent',
                                border: '2px dashed rgba(0,0,0,0.2)',
                            }}
                        >
                            <AddIcon sx={{ color: 'rgba(0,0,0,0.4)' }} />
                        </Avatar>
                        <Typography variant="body2" sx={{ color: 'rgba(0,0,0,0.6)' }}>
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
                                    background: '#f5f5f5',
                                    border: `1px solid ${avatarImage ? '#a67c00' : 'rgba(0,0,0,0.12)'}`,
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
                                            color: 'rgba(0,0,0,0.7)',
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
                                border: '1px solid rgba(0,0,0,0.23)',
                                borderRadius: '4px',
                                marginBottom: '12px',
                                boxSizing: 'border-box',
                                outline: 'none',
                            }}
                            onFocus={(e) => e.target.style.borderColor = '#a67c00'}
                            onBlur={(e) => e.target.style.borderColor = 'rgba(0,0,0,0.23)'}
                        />
                        
                        {/* Color Picker */}
                        <Typography variant="caption" sx={{ color: 'rgba(0,0,0,0.5)', mb: 0.5, display: 'block' }}>
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
                                        border: selectedColor === color ? '2px solid #000' : '2px solid transparent',
                                        '&:hover': {
                                            transform: 'scale(1.1)',
                                        },
                                    }}
                                />
                            ))}
                        </Box>

                        <Box sx={{ display: 'flex', gap: 1 }}>
                            <button
                                onClick={handleCancelCreate}
                                style={{
                                    padding: '6px 16px',
                                    fontSize: '13px',
                                    border: '1px solid rgba(0,0,0,0.23)',
                                    borderRadius: '4px',
                                    background: 'white',
                                    cursor: 'pointer',
                                }}
                            >
                                Cancel
                            </button>
                            <button
                                onClick={handleCreateProfile}
                                disabled={!newProfileName.trim()}
                                style={{
                                    padding: '6px 16px',
                                    fontSize: '13px',
                                    border: 'none',
                                    borderRadius: '4px',
                                    background: newProfileName.trim() ? '#a67c00' : '#ccc',
                                    color: 'white',
                                    cursor: newProfileName.trim() ? 'pointer' : 'not-allowed',
                                }}
                            >
                                Create
                            </button>
                        </Box>
                    </Box>
                )}
            </Box>

            {/* Footer hint */}
            <Box sx={{
                p: 1,
                borderTop: '1px solid rgba(0,0,0,0.08)',
                bgcolor: 'rgba(0,0,0,0.02)',
            }}>
                <Typography variant="caption" sx={{ color: 'rgba(0,0,0,0.5)', display: 'block', textAlign: 'center' }}>
                    Switching profiles opens a new window
                </Typography>
            </Box>
        </Box>
    );
};

export default ProfilePickerOverlayRoot;
