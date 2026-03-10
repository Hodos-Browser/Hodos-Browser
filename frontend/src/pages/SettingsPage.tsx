import React, { useState, useEffect } from 'react';
import { useParams } from 'react-router-dom';
import {
  Box,
  Typography,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
} from '@mui/material';
import LanguageIcon from '@mui/icons-material/Language';
import ShieldIcon from '@mui/icons-material/Shield';
import DownloadIcon from '@mui/icons-material/Download';
import AccountBalanceWalletIcon from '@mui/icons-material/AccountBalanceWallet';
import OpenInNewIcon from '@mui/icons-material/OpenInNew';
import InfoIcon from '@mui/icons-material/Info';
import GeneralSettings from '../components/settings/GeneralSettings';
import PrivacySettings from '../components/settings/PrivacySettings';
import DownloadSettings from '../components/settings/DownloadSettings';
import AboutSettings from '../components/settings/AboutSettings';

interface Section {
  id: string;
  label: string;
  icon: React.ReactNode;
  externalAction?: string; // IPC menu_action to open in new tab instead of inline
}

const sections: Section[] = [
  { id: 'general', label: 'General', icon: <LanguageIcon /> },
  { id: 'privacy', label: 'Privacy & Security', icon: <ShieldIcon /> },
  { id: 'downloads', label: 'Downloads', icon: <DownloadIcon /> },
  { id: 'wallet', label: 'Wallet', icon: <AccountBalanceWalletIcon />, externalAction: 'wallet' },
  { id: 'about', label: 'About Hodos', icon: <InfoIcon /> },
];

const SettingsPage: React.FC = () => {
  const { section: urlSection } = useParams<{ section?: string }>();
  const [activeSection, setActiveSection] = useState(urlSection || 'general');

  useEffect(() => {
    document.title = 'Hodos Settings';
    // Reset body margin/overflow so 100vh container fits without double scrollbar
    document.body.style.margin = '0';
    document.body.style.overflow = 'hidden';
  }, []);

  return (
    <Box sx={{ display: 'flex', height: '100vh', overflow: 'hidden', bgcolor: '#121212', color: '#e0e0e0' }}>
      {/* Sidebar */}
      <Box
        sx={{
          width: 240,
          borderRight: '1px solid #333',
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
            Settings
          </Typography>
        </Box>
        <List sx={{ px: 1 }}>
          {sections.map((section) => (
            <ListItemButton
              key={section.id}
              selected={!section.externalAction && activeSection === section.id}
              onClick={() => {
                if (section.externalAction) {
                  window.cefMessage?.send('menu_action', [section.externalAction]);
                } else {
                  setActiveSection(section.id);
                }
              }}
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
              {section.externalAction && (
                <OpenInNewIcon sx={{ fontSize: 14, color: '#666' }} />
              )}
            </ListItemButton>
          ))}
        </List>
      </Box>

      {/* Main Content — single scroll container */}
      <Box
        sx={{
          flex: 1,
          overflowY: 'auto',
          overflowX: 'hidden',
        }}
      >
        <Box sx={{ maxWidth: 780, mx: 'auto', p: 4 }}>
          {activeSection === 'general' && <GeneralSettings />}
          {activeSection === 'privacy' && <PrivacySettings />}
          {activeSection === 'downloads' && <DownloadSettings />}
          {activeSection === 'about' && <AboutSettings />}
        </Box>
      </Box>
    </Box>
  );
};

export default SettingsPage;
