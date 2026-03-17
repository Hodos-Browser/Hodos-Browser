import { useState, useEffect } from 'react';
import {
  Box,
  Typography,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
} from '@mui/material';
import {
  History as HistoryIcon,
  Cookie,
  Storage,
} from '@mui/icons-material';
import { useSearchParams } from 'react-router-dom';
import { HistoryPanel } from '../components/HistoryPanel';
import { CookiesPanel } from '../components/CookiesPanel';
import { CachePanel } from '../components/CachePanel';

interface Section {
  id: string;
  label: string;
  icon: React.ReactNode;
}

const sections: Section[] = [
  { id: 'history', label: 'History', icon: <HistoryIcon /> },
  { id: 'cookies', label: 'Cookies', icon: <Cookie /> },
  { id: 'cache', label: 'Cache & Storage', icon: <Storage /> },
];

export function HistoryPage() {
  const [searchParams] = useSearchParams();
  const initialSection = searchParams.get('tab') === 'cookies' ? 'cookies' : searchParams.get('tab') === 'cache' ? 'cache' : 'history';
  const [activeSection, setActiveSection] = useState(initialSection);

  useEffect(() => {
    document.title = 'Hodos Browser Data';
    document.body.style.margin = '0';
    document.body.style.overflow = 'hidden';
  }, []);

  return (
    <Box sx={{ display: 'flex', height: '100vh', overflow: 'hidden', bgcolor: '#0f1117', color: '#e0e0e0' }}>
      {/* Sidebar */}
      <Box
        sx={{
          width: 240,
          bgcolor: '#111827',
          borderRight: '1px solid #2a2d35',
          py: 2,
          overflowY: 'auto',
          flexShrink: 0,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, px: 2, mb: 2 }}>
          <img src="/Hodos_Gold_Icon.svg" alt="Hodos" style={{ width: 24, height: 24 }} />
          <Typography
            variant="h6"
            sx={{
              color: '#a67c00',
              fontWeight: 600,
              fontSize: '1.1rem',
            }}
          >
            Browser Data
          </Typography>
        </Box>
        <List sx={{ px: 1 }}>
          {sections.map((section) => (
            <ListItemButton
              key={section.id}
              selected={activeSection === section.id}
              onClick={() => setActiveSection(section.id)}
              sx={{
                borderRadius: 1,
                mb: 0.5,
                py: 1,
                '&.Mui-selected': {
                  bgcolor: 'rgba(166, 124, 0, 0.15)',
                  color: '#a67c00',
                  '&:hover': { bgcolor: 'rgba(166, 124, 0, 0.2)' },
                },
                '&:hover': { bgcolor: 'rgba(255,255,255,0.05)' },
              }}
            >
              <ListItemIcon sx={{ minWidth: 36, color: 'inherit' }}>
                {section.icon}
              </ListItemIcon>
              <ListItemText
                primary={section.label}
                primaryTypographyProps={{ fontSize: '0.88rem' }}
              />
            </ListItemButton>
          ))}
        </List>
      </Box>

      {/* Main Content */}
      <Box
        sx={{
          flex: 1,
          overflowY: 'auto',
          overflowX: 'hidden',
        }}
      >
        <Box sx={{ maxWidth: 780, mx: 'auto', p: 4 }}>
          {activeSection === 'history' && <HistoryPanel />}
          {activeSection === 'cookies' && <CookiesPanel />}
          {activeSection === 'cache' && <CachePanel />}
        </Box>
      </Box>
    </Box>
  );
}

export default HistoryPage;
