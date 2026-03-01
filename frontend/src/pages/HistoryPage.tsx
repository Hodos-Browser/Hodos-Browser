import { useState, useEffect } from 'react';
import { Box, Container, Typography, Paper, Tabs, Tab } from '@mui/material';
import {
  History as HistoryIcon,
  Cookie,
  Storage,
} from '@mui/icons-material';
import { useSearchParams } from 'react-router-dom';
import { HistoryPanel } from '../components/HistoryPanel';
import { CookiesPanel } from '../components/CookiesPanel';
import { CachePanel } from '../components/CachePanel';

export function HistoryPage() {
  const [searchParams] = useSearchParams();
  const initialTab = searchParams.get('tab') === 'cookies' ? 1 : searchParams.get('tab') === 'cache' ? 2 : 0;
  const [tabIndex, setTabIndex] = useState(initialTab);

  useEffect(() => {
    document.title = 'Hodos Browser Data';
    document.body.style.margin = '0';
    document.body.style.overflow = 'hidden';
  }, []);

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
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5 }}>
              <img src="/Hodos_Black_Icon.svg" alt="" style={{ height: 40 }} />
              <Typography variant="h4" component="h1" sx={{ fontWeight: 500 }}>
                Browsing Data
              </Typography>
            </Box>
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
