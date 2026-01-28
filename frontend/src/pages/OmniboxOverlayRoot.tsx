import React from 'react';
import { Box, Typography, Paper } from '@mui/material';

/**
 * OmniboxOverlayRoot - Root component for the omnibox autocomplete overlay.
 *
 * Phase 1: Placeholder UI to verify overlay infrastructure works.
 * Phase 2: Will be replaced with suggestion list rendering.
 */
const OmniboxOverlayRoot: React.FC = () => {
    React.useEffect(() => {
        console.log("🔍 OmniboxOverlayRoot mounted");
    }, []);

    return (
        <Box
            sx={{
                width: '100%',
                height: '100%',
                backgroundColor: 'background.paper',
                boxShadow: 3,
                borderRadius: 1,
                overflow: 'hidden',
            }}
        >
            <Paper
                elevation={3}
                sx={{
                    p: 2,
                    backgroundColor: 'background.default',
                }}
            >
                <Typography variant="body2" color="text.secondary">
                    Omnibox Overlay (Phase 1 - Infrastructure)
                </Typography>
                <Typography variant="caption" color="text.disabled" sx={{ mt: 1, display: 'block' }}>
                    Placeholder UI. Phase 2 will render suggestions here.
                </Typography>
            </Paper>
        </Box>
    );
};

export default OmniboxOverlayRoot;
