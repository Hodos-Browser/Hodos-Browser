import { useState, useEffect, useCallback, useMemo } from 'react';

export interface DownloadItem {
    id: number;
    url: string;
    filename: string;
    fullPath: string;
    receivedBytes: number;
    totalBytes: number;
    percentComplete: number;
    currentSpeed: number;
    isInProgress: boolean;
    isComplete: boolean;
    isCanceled: boolean;
    isPaused: boolean;
}

export function useDownloads() {
    const [downloads, setDownloads] = useState<DownloadItem[]>([]);

    useEffect(() => {
        const handler = (event: MessageEvent) => {
            if (event.data?.type === 'download_state_update') {
                try {
                    const parsed = typeof event.data.data === 'string'
                        ? JSON.parse(event.data.data)
                        : event.data.data;
                    setDownloads(parsed as DownloadItem[]);
                } catch (e) {
                    console.error('Failed to parse download state:', e);
                }
            }
        };

        window.addEventListener('message', handler);

        // Request initial state
        window.cefMessage?.send('download_get_state');

        return () => window.removeEventListener('message', handler);
    }, []);

    const cancelDownload = useCallback((id: number) => {
        window.cefMessage?.send('download_cancel', [id.toString()]);
    }, []);

    const pauseDownload = useCallback((id: number) => {
        window.cefMessage?.send('download_pause', [id.toString()]);
    }, []);

    const resumeDownload = useCallback((id: number) => {
        window.cefMessage?.send('download_resume', [id.toString()]);
    }, []);

    const openFile = useCallback((id: number) => {
        window.cefMessage?.send('download_open', [id.toString()]);
    }, []);

    const showInFolder = useCallback((id: number) => {
        window.cefMessage?.send('download_show_folder', [id.toString()]);
    }, []);

    const clearCompleted = useCallback(() => {
        window.cefMessage?.send('download_clear_completed');
    }, []);

    const hasActiveDownloads = useMemo(
        () => downloads.some(d => d.isInProgress || d.isPaused),
        [downloads]
    );

    const hasDownloads = downloads.length > 0;

    return {
        downloads,
        hasDownloads,
        hasActiveDownloads,
        cancelDownload,
        pauseDownload,
        resumeDownload,
        openFile,
        showInFolder,
        clearCompleted,
    };
}
