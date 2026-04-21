import React, { useState, useMemo } from 'react';
import { useSearchParams } from 'react-router-dom';
import { Box, Typography, Button } from '@mui/material';

const ERROR_DESCRIPTIONS: Record<string, { title: string; detail: string }> = {
  name_mismatch: {
    title: 'The certificate does not match this site',
    detail:
      'The security certificate presented by this site was issued for a different domain. This could mean someone is trying to impersonate the site.',
  },
  date_invalid: {
    title: 'The certificate has expired or is not yet valid',
    detail:
      "The site's security certificate has an invalid date. It may have expired or been set to a future date.",
  },
  authority_invalid: {
    title: 'The certificate authority is not trusted',
    detail:
      "The site's security certificate was not issued by a trusted certificate authority. The connection may not be secure.",
  },
  revoked: {
    title: 'The certificate has been revoked',
    detail:
      "The site's security certificate has been revoked by its issuer. This site should not be trusted.",
  },
  invalid: {
    title: 'The certificate is invalid',
    detail:
      "The site's security certificate contains errors that prevent it from being verified.",
  },
  unknown: {
    title: 'There is a problem with the security certificate',
    detail:
      "The site's security certificate could not be verified. The connection may not be secure.",
  },
};

const CertErrorPage: React.FC = () => {
  const [searchParams] = useSearchParams();
  const [showAdvanced, setShowAdvanced] = useState(false);

  const domain = searchParams.get('domain') || 'this site';
  const errorType = searchParams.get('error') || 'unknown';
  const originalUrl = searchParams.get('url') || '';
  const errorCode = searchParams.get('code') || '';

  const errorInfo = useMemo(
    () => ERROR_DESCRIPTIONS[errorType] || ERROR_DESCRIPTIONS.unknown,
    [errorType]
  );

  const handleGoBack = () => {
    if (window.cefMessage) {
      window.cefMessage.send('cert_error_go_back', []);
    }
  };

  const handleProceed = () => {
    if (window.cefMessage && originalUrl) {
      window.cefMessage.send('cert_error_proceed', [originalUrl]);
    }
  };

  return (
    <Box
      sx={{
        minHeight: '100vh',
        bgcolor: '#1a1a1a',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        px: 3,
        fontFamily: 'Inter, system-ui, sans-serif',
      }}
    >
      <Box sx={{ maxWidth: 560, width: '100%', textAlign: 'center' }}>
        {/* Warning icon */}
        <Box sx={{ mb: 3 }}>
          <svg
            width="72"
            height="72"
            viewBox="0 0 24 24"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              d="M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z"
              fill="#a67c00"
            />
          </svg>
        </Box>

        <Typography
          variant="h5"
          sx={{
            color: '#ffffff',
            fontWeight: 600,
            mb: 1.5,
            fontSize: '1.4rem',
          }}
        >
          Your connection is not private
        </Typography>

        <Typography
          sx={{
            color: 'rgba(255,255,255,0.7)',
            mb: 1,
            fontSize: '0.95rem',
            lineHeight: 1.6,
          }}
        >
          Attackers might be trying to steal your information from{' '}
          <strong style={{ color: '#ffffff' }}>{domain}</strong> (for example,
          passwords, messages, or credit cards).
        </Typography>

        <Typography
          sx={{
            color: 'rgba(255,255,255,0.5)',
            mb: 3,
            fontSize: '0.85rem',
          }}
        >
          {errorInfo.title}
          {errorCode ? ` (NET::ERR ${errorCode})` : ''}
        </Typography>

        <Button
          variant="contained"
          onClick={handleGoBack}
          sx={{
            bgcolor: '#a67c00',
            color: '#ffffff',
            fontWeight: 600,
            textTransform: 'none',
            fontSize: '0.95rem',
            px: 4,
            py: 1.2,
            borderRadius: '8px',
            mb: 2,
            '&:hover': {
              bgcolor: '#8a6700',
            },
          }}
        >
          Go back to safety
        </Button>

        {/* Advanced section */}
        <Box sx={{ mt: 2 }}>
          <Button
            variant="text"
            onClick={() => setShowAdvanced(!showAdvanced)}
            sx={{
              color: 'rgba(255,255,255,0.5)',
              textTransform: 'none',
              fontSize: '0.85rem',
              '&:hover': {
                color: 'rgba(255,255,255,0.7)',
                bgcolor: 'transparent',
              },
            }}
          >
            {showAdvanced ? 'Hide advanced' : 'Advanced...'}
          </Button>

          {showAdvanced && (
            <Box
              sx={{
                mt: 2,
                p: 2.5,
                bgcolor: 'rgba(255,255,255,0.05)',
                borderRadius: '8px',
                textAlign: 'left',
              }}
            >
              <Typography
                sx={{
                  color: 'rgba(255,255,255,0.6)',
                  fontSize: '0.85rem',
                  lineHeight: 1.6,
                  mb: 2,
                }}
              >
                {errorInfo.detail}
              </Typography>

              <Button
                variant="text"
                onClick={handleProceed}
                sx={{
                  color: 'rgba(255,255,255,0.4)',
                  textTransform: 'none',
                  fontSize: '0.85rem',
                  p: 0,
                  '&:hover': {
                    color: 'rgba(255,255,255,0.6)',
                    bgcolor: 'transparent',
                  },
                }}
              >
                Proceed to {domain} (unsafe)
              </Button>
            </Box>
          )}
        </Box>
      </Box>
    </Box>
  );
};

export default CertErrorPage;
