import { useState } from 'react';
import { Box, Container, Typography, Paper, Tabs, Tab } from '@mui/material';
import {
  History as HistoryIcon,
  Cookie,
  Storage,
} from '@mui/icons-material';
import { HistoryPanel } from '../components/HistoryPanel';
import { CookiesPanel } from '../components/CookiesPanel';
import { CachePanel } from '../components/CachePanel';

export function HistoryPage() {
  const [tabIndex, setTabIndex] = useState(0);

  return (
    <Box
      sx={{
        minHeight: '100vh',
        bgcolor: '#f5f5f5',
        py: 3,
      }}
    >
      <Container maxWidth="lg">
        <Paper
          elevation={0}
          sx={{
            bgcolor: 'white',
            borderRadius: 2,
            overflow: 'hidden',
            minHeight: '80vh',
          }}
        >
          <Box sx={{ borderBottom: '1px solid rgba(0, 0, 0, 0.12)', p: 3, pb: 0, bgcolor: '#fafafa' }}>
            <Typography variant="h4" component="h1" sx={{ fontWeight: 500 }}>
              Browsing Data
            </Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5, mb: 2 }}>
              View and manage your browsing history, cookies, and cache
            </Typography>
            <Tabs
              value={tabIndex}
              onChange={(_e, newValue) => setTabIndex(newValue)}
              sx={{
                '& .MuiTab-root': {
                  textTransform: 'none',
                  minHeight: 48,
                  fontWeight: 500,
                },
              }}
            >
              <Tab icon={<HistoryIcon />} iconPosition="start" label="History" />
              <Tab icon={<Cookie />} iconPosition="start" label="Cookies" />
              <Tab icon={<Storage />} iconPosition="start" label="Cache" />
            </Tabs>
          </Box>

          {tabIndex === 0 && <HistoryPanel />}
          {tabIndex === 1 && <CookiesPanel />}
          {tabIndex === 2 && <CachePanel />}
        </Paper>
      </Container>
    </Box>
  );
}

export default HistoryPage;
