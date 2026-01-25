import React, { useState } from 'react';
import {
  Box,
  Paper,
  InputBase,
  List,
  ListItem,
  Fade,
} from '@mui/material';

interface OmniboxProps {
  onNavigate: (url: string) => void;
  initialValue?: string;
}

const Omnibox: React.FC<OmniboxProps> = ({ onNavigate, initialValue = '' }) => {
  const [inputValue, setInputValue] = useState<string>(initialValue);
  const [showDropdown, setShowDropdown] = useState<boolean>(false);

  const mockSuggestions: string[] = [
    'https://google.com',
    'https://github.com',
    'https://wikipedia.org',
    'https://stackoverflow.com',
    'https://reddit.com',
  ];

  const filteredSuggestions = mockSuggestions.filter((suggestion) =>
    suggestion.toLowerCase().includes(inputValue.toLowerCase())
  );

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setInputValue(value);
    setShowDropdown(value.length > 0);
  };

  const handleInputFocus = (e: React.FocusEvent<HTMLInputElement>) => {
    e.target.select();
  };

  const handleInputBlur = () => {
    setTimeout(() => {
      setShowDropdown(false);
    }, 200);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      onNavigate(inputValue);
      setShowDropdown(false);
    } else if (e.key === 'Escape') {
      setInputValue('');
      setShowDropdown(false);
    }
  };

  const handleSuggestionClick = (suggestion: string) => {
    onNavigate(suggestion);
    setInputValue(suggestion);
    setShowDropdown(false);
  };

  return (
    <Box sx={{ position: 'relative', flex: 1, minWidth: 0 }}>
      <Paper
        sx={{
          display: 'flex',
          alignItems: 'center',
          height: 36,
          borderRadius: 20,
          px: 2,
          bgcolor: '#f1f3f4',
          boxShadow: 'none',
          border: '1px solid transparent',
          '&:hover': {
            bgcolor: '#ffffff',
            border: '1px solid rgba(0, 0, 0, 0.1)',
          },
          '&:focus-within': {
            bgcolor: '#ffffff',
            border: '1px solid #1a73e8',
            boxShadow: '0 0 0 2px rgba(26, 115, 232, 0.1)',
          },
        }}
      >
        <InputBase
          value={inputValue}
          onChange={handleInputChange}
          onFocus={handleInputFocus}
          onBlur={handleInputBlur}
          onKeyDown={handleKeyDown}
          placeholder="Search or enter address"
          fullWidth
          sx={{
            fontSize: 14,
            color: 'rgba(0, 0, 0, 0.87)',
            '& input': {
              padding: 0,
              '&::placeholder': {
                color: 'rgba(0, 0, 0, 0.4)',
                opacity: 1,
              },
            },
          }}
        />
      </Paper>

      <Fade in={showDropdown && filteredSuggestions.length > 0}>
        <Paper
          sx={{
            position: 'absolute',
            top: 'calc(100% + 8px)',
            left: 0,
            right: 0,
            zIndex: 1000,
            borderRadius: 2,
            boxShadow: 8,
          }}
        >
          <List sx={{ py: 1 }}>
            {filteredSuggestions.map((suggestion, index) => (
              <ListItem
                key={index}
                onClick={() => handleSuggestionClick(suggestion)}
                sx={{
                  cursor: 'pointer',
                  px: 2,
                  py: 1,
                  '&:hover': {
                    bgcolor: '#f5f5f5',
                  },
                }}
              >
                {suggestion}
              </ListItem>
            ))}
          </List>
        </Paper>
      </Fade>
    </Box>
  );
};

export default Omnibox;
