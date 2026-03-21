import React, { useState, useRef, useEffect, useCallback } from 'react';
import { Box, Typography } from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import KeyboardArrowUpIcon from '@mui/icons-material/KeyboardArrowUp';
import KeyboardArrowDownIcon from '@mui/icons-material/KeyboardArrowDown';
import { HodosButton } from './HodosButton';

interface FindBarProps {
  onClose: () => void;
  findResult: { count: number; activeMatch: number } | null;
}

const FindBar: React.FC<FindBarProps> = ({ onClose, findResult }) => {
  const [query, setQuery] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  // Auto-focus input when mounted
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const doFind = useCallback((text: string, forward: boolean, findNext: boolean) => {
    if (window.cefMessage) {
      window.cefMessage.send('find_text', [text, forward, false, findNext]);
    }
  }, []);

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newQuery = e.target.value;
    setQuery(newQuery);
    doFind(newQuery, true, false);
  };

  const handleNext = useCallback(() => {
    if (query) doFind(query, true, true);
  }, [query, doFind]);

  const handlePrev = useCallback(() => {
    if (query) doFind(query, false, true);
  }, [query, doFind]);

  const handleClose = useCallback(() => {
    if (window.cefMessage) {
      window.cefMessage.send('find_stop');
    }
    onClose();
  }, [onClose]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      handleClose();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (e.shiftKey) {
        handlePrev();
      } else {
        handleNext();
      }
    }
  };

  const matchText = (() => {
    if (!query) return '';
    if (!findResult) return '';
    if (findResult.count === 0) return 'No matches';
    return `${findResult.activeMatch} of ${findResult.count}`;
  })();

  return (
    <Box
      sx={{
        flexShrink: 0,
        display: 'flex',
        alignItems: 'center',
        gap: 0.5,
        bgcolor: '#f0f0f0',
        border: '1px solid rgba(0,0,0,0.15)',
        borderRadius: '6px',
        px: 0.75,
        ml: 0.5,
        height: 34,
      }}
    >
      <input
        ref={inputRef}
        type="text"
        value={query}
        onChange={handleInputChange}
        onKeyDown={handleKeyDown}
        placeholder="Find in page"
        style={{
          width: 160,
          height: 24,
          border: '1px solid rgba(0,0,0,0.15)',
          borderRadius: 4,
          paddingLeft: 6,
          paddingRight: 6,
          fontSize: 13,
          outline: 'none',
          backgroundColor: findResult && query && findResult.count === 0 ? '#fff0f0' : '#fff',
        }}
      />
      {matchText && (
        <Typography
          variant="caption"
          sx={{
            color: findResult && findResult.count === 0 ? '#d93025' : 'rgba(0,0,0,0.54)',
            whiteSpace: 'nowrap',
            fontSize: 11,
            minWidth: 50,
            textAlign: 'center',
          }}
        >
          {matchText}
        </Typography>
      )}
      <HodosButton variant="icon" size="small" onClick={handlePrev} disabled={!query || !findResult || findResult.count === 0} aria-label="Previous match">
        <KeyboardArrowUpIcon sx={{ fontSize: 18 }} />
      </HodosButton>
      <HodosButton variant="icon" size="small" onClick={handleNext} disabled={!query || !findResult || findResult.count === 0} aria-label="Next match">
        <KeyboardArrowDownIcon sx={{ fontSize: 18 }} />
      </HodosButton>
      <HodosButton variant="icon" size="small" onClick={handleClose} aria-label="Close find bar">
        <CloseIcon sx={{ fontSize: 16 }} />
      </HodosButton>
    </Box>
  );
};

export default FindBar;
