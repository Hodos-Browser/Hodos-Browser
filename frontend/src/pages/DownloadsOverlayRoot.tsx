import React from 'react';
import { Box, Typography, LinearProgress, Link } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import PauseIcon from '@mui/icons-material/Pause';
import PlayArrowIcon from '@mui/icons-material/PlayArrow';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import { useDownloads } from '../hooks/useDownloads';
import type { DownloadItem } from '../hooks/useDownloads';
import { HodosButton } from '../components/HodosButton';

function formatBytes(bytes: number): string {
    if (bytes <= 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    const val = bytes / Math.pow(1024, i);
    return val.toFixed(i > 0 ? 1 : 0) + ' ' + units[i];
}

function formatSpeed(bytesPerSec: number): string {
    return formatBytes(bytesPerSec) + '/s';
}

function truncateFilename(name: string, max: number = 30): string {
    if (name.length <= max) return name;
    const ext = name.lastIndexOf('.');
    if (ext > 0 && name.length - ext <= 6) {
        const extStr = name.slice(ext);
        return name.slice(0, max - extStr.length - 3) + '...' + extStr;
    }
    return name.slice(0, max - 3) + '...';
}

const DownloadsOverlayRoot: React.FC = () => {
    const {
        downloads,
        cancelDownload,
        pauseDownload,
        resumeDownload,
        openFile,
        showInFolder,
        clearCompleted,
    } = useDownloads();

    const hasCompletedOrCanceled = downloads.some(d => d.isComplete || d.isCanceled);
    const hasActiveDownloads = downloads.some(d => d.isInProgress || d.isPaused);

    const handleClearCompleted = () => {
        clearCompleted();
        // If no active downloads remain, clearing will empty the list — close the overlay
        if (!hasActiveDownloads) {
            window.cefMessage?.send('download_panel_hide');
        }
    };

    return (
        <Box sx={{
            width: '100%',
            height: '100%',
            bgcolor: '#1a1d23',
            borderRadius: '8px',
            boxShadow: '0 4px 20px rgba(0,0,0,0.15)',
            overflow: 'hidden',
            display: 'flex',
            flexDirection: 'column',
        }}>
            <Box sx={{
                p: 1.5,
                borderBottom: '1px solid #2a2d35',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
            }}>
                <Typography variant="subtitle2" sx={{ fontWeight: 600, color: '#f0f0f0' }}>
                    Downloads
                </Typography>
                <HodosButton variant="icon" size="small" onClick={() => window.cefMessage?.send('download_panel_hide')} aria-label="Close">
                    <CloseIcon sx={{ fontSize: 16 }} />
                </HodosButton>
            </Box>

            <Box sx={{ flex: 1, overflow: 'auto', p: 1.5, pt: 0.5 }}>
                {downloads.length === 0 && (
                    <Typography variant="body2" sx={{ textAlign: 'center', py: 3, color: '#6b7280' }}>
                        No downloads
                    </Typography>
                )}

                {downloads.map((dl: DownloadItem) => (
                    <Box
                        key={dl.id}
                        sx={{
                            py: 1,
                            px: 0.5,
                            borderBottom: '1px solid #2a2d35',
                            '&:last-child': { borderBottom: 'none' },
                        }}
                    >
                        <Typography
                            variant="body2"
                            sx={{
                                fontWeight: 500,
                                color: '#f0f0f0',
                                mb: 0.5,
                                overflow: 'hidden',
                                textOverflow: 'ellipsis',
                                whiteSpace: 'nowrap',
                            }}
                            title={dl.filename}
                        >
                            {truncateFilename(dl.filename)}
                        </Typography>

                        {/* In Progress */}
                        {dl.isInProgress && (
                            <>
                                <LinearProgress
                                    variant={dl.totalBytes > 0 ? 'determinate' : 'indeterminate'}
                                    value={dl.percentComplete >= 0 ? dl.percentComplete : 0}
                                    sx={{ mb: 0.5, height: 4, borderRadius: 2 }}
                                />
                                <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                                    <Typography variant="caption" sx={{ color: '#9ca3af' }}>
                                        {formatBytes(dl.receivedBytes)}
                                        {dl.totalBytes > 0 ? ` / ${formatBytes(dl.totalBytes)}` : ''}
                                        {dl.currentSpeed > 0 ? ` — ${formatSpeed(dl.currentSpeed)}` : ''}
                                    </Typography>
                                    <Box>
                                        <HodosButton variant="icon" size="small" onClick={() => pauseDownload(dl.id)} aria-label="Pause">
                                            <PauseIcon sx={{ fontSize: 16 }} />
                                        </HodosButton>
                                        <HodosButton variant="icon" size="small" onClick={() => cancelDownload(dl.id)} aria-label="Cancel">
                                            <CloseIcon sx={{ fontSize: 16 }} />
                                        </HodosButton>
                                    </Box>
                                </Box>
                            </>
                        )}

                        {/* Paused */}
                        {dl.isPaused && (
                            <>
                                <LinearProgress
                                    variant="determinate"
                                    value={dl.percentComplete >= 0 ? dl.percentComplete : 0}
                                    sx={{ mb: 0.5, height: 4, borderRadius: 2 }}
                                    color="warning"
                                />
                                <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                                    <Typography variant="caption" sx={{ color: '#ed6c02' }}>
                                        Paused — {formatBytes(dl.receivedBytes)}
                                        {dl.totalBytes > 0 ? ` / ${formatBytes(dl.totalBytes)}` : ''}
                                    </Typography>
                                    <Box>
                                        <HodosButton variant="icon" size="small" onClick={() => resumeDownload(dl.id)} aria-label="Resume">
                                            <PlayArrowIcon sx={{ fontSize: 16 }} />
                                        </HodosButton>
                                        <HodosButton variant="icon" size="small" onClick={() => cancelDownload(dl.id)} aria-label="Cancel">
                                            <CloseIcon sx={{ fontSize: 16 }} />
                                        </HodosButton>
                                    </Box>
                                </Box>
                            </>
                        )}

                        {/* Complete */}
                        {dl.isComplete && (
                            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                                <CheckCircleIcon sx={{ fontSize: 16, color: '#2e7d32' }} />
                                <Typography variant="caption" sx={{ color: '#9ca3af' }}>
                                    {formatBytes(dl.totalBytes > 0 ? dl.totalBytes : dl.receivedBytes)}
                                </Typography>
                                <Link
                                    component="button"
                                    variant="caption"
                                    onClick={() => openFile(dl.id)}
                                    sx={{ cursor: 'pointer' }}
                                >
                                    Open
                                </Link>
                                <Link
                                    component="button"
                                    variant="caption"
                                    onClick={() => showInFolder(dl.id)}
                                    sx={{ cursor: 'pointer' }}
                                >
                                    Show in folder
                                </Link>
                            </Box>
                        )}

                        {/* Canceled */}
                        {dl.isCanceled && !dl.isComplete && (
                            <Typography variant="caption" sx={{ color: '#6b7280' }}>
                                Canceled
                            </Typography>
                        )}
                    </Box>
                ))}

                {hasCompletedOrCanceled && (
                    <Box sx={{ mt: 1, textAlign: 'center' }}>
                        <HodosButton variant="ghost" size="small" onClick={handleClearCompleted}>
                            Clear completed
                        </HodosButton>
                    </Box>
                )}
            </Box>
        </Box>
    );
};

export default DownloadsOverlayRoot;
