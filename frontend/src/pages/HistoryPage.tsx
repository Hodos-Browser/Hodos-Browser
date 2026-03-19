import { useState, useEffect } from 'react';
import {
  Box,
  Typography,
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
          width: 220,
          minWidth: 220,
          bgcolor: '#111827',
          borderRight: '1px solid #2a2d35',
          display: 'flex',
          flexDirection: 'column',
          overflowY: 'auto',
          flexShrink: 0,
        }}
      >
        <Box sx={{ height: 56, px: 2, borderBottom: '1px solid #2a2d35', display: 'flex', alignItems: 'center', gap: 1, boxSizing: 'border-box' }}>
          <img src="/Hodos_Gold_Icon.svg" alt="Hodos" style={{ width: 24, height: 24 }} />
          <Typography
            sx={{
              color: '#a67c00',
              fontWeight: 600,
              fontSize: '1.1rem',
            }}
          >
            Browser Data
          </Typography>
        </Box>
        <Box sx={{ py: 1.5, flex: 1 }}>
          {sections.map((section) => {
            const isActive = activeSection === section.id;
            return (
              <Box
                key={section.id}
                onClick={() => setActiveSection(section.id)}
                sx={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 1.5,
                  px: 2.5,
                  py: 1.5,
                  cursor: 'pointer',
                  color: isActive ? '#a67c00' : '#9ca3af',
                  fontSize: '0.88rem',
                  fontWeight: isActive ? 600 : 500,
                  borderLeft: `3px solid ${isActive ? '#a67c00' : 'transparent'}`,
                  bgcolor: isActive ? '#1a1a2e' : 'transparent',
                  transition: 'all 0.15s ease',
                  userSelect: 'none',
                  '&:hover': {
                    bgcolor: isActive ? '#1a1a2e' : '#1f2937',
                    color: isActive ? '#a67c00' : '#f0f0f0',
                  },
                }}
              >
                <Box sx={{ width: 24, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'inherit', fontSize: 18 }}>
                  {section.icon}
                </Box>
                <Box sx={{ flex: 1 }}>{section.label}</Box>
              </Box>
            );
          })}
        </Box>
      </Box>

      {/* Main Content */}
      <Box
        sx={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
        }}
      >
        {/* Content Header */}
        <Box
          sx={{
            height: 56,
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            px: 3,
            borderBottom: '1px solid #2a2d35',
            bgcolor: '#111827',
            boxSizing: 'border-box',
            flexShrink: 0,
          }}
        >
          <Typography sx={{ fontSize: 18, fontWeight: 600, color: '#f0f0f0' }}>
            {sections.find(s => s.id === activeSection)?.label || 'Browser Data'}
          </Typography>
        </Box>

        <Box sx={{ flex: 1, overflowY: 'auto', overflowX: 'hidden' }}>
          <Box sx={{ maxWidth: 780, mx: 'auto', p: 4 }}>
            {activeSection === 'history' && <HistoryPanel />}
            {activeSection === 'cookies' && <CookiesPanel />}
            {activeSection === 'cache' && <CachePanel />}
          </Box>
        </Box>
      </Box>
    </Box>
  );
}

export default HistoryPage;
