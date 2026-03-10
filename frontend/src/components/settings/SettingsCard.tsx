import React from 'react';
import { Box, Typography, Paper } from '@mui/material';

interface SettingsCardProps {
  title: string;
  children: React.ReactNode;
}

export const SettingsCard: React.FC<SettingsCardProps> = ({ title, children }) => (
  <Paper sx={{ p: 3, mb: 3, bgcolor: '#1e1e1e', borderRadius: 2 }}>
    <Typography variant="h6" sx={{ mb: 2, color: '#a67c00', fontSize: '1rem' }}>
      {title}
    </Typography>
    {children}
  </Paper>
);

interface SettingRowProps {
  label: string;
  description?: string;
  control: React.ReactNode;
}

export const SettingRow: React.FC<SettingRowProps> = ({ label, description, control }) => (
  <Box
    sx={{
      display: 'flex',
      justifyContent: 'space-between',
      alignItems: 'center',
      py: 1.5,
      borderBottom: '1px solid #333',
      '&:last-child': { borderBottom: 'none' },
    }}
  >
    <Box sx={{ mr: 2 }}>
      <Typography variant="body1" sx={{ color: '#e0e0e0', fontSize: '0.9rem' }}>
        {label}
      </Typography>
      {description && (
        <Typography variant="body2" sx={{ color: '#888', mt: 0.25, fontSize: '0.78rem' }}>
          {description}
        </Typography>
      )}
    </Box>
    {control}
  </Box>
);
