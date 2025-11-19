import React, { useState } from 'react';
import {
  Modal,
  Box,
  Typography,
  Button,
  IconButton,
  Collapse,
  TextField,
  Checkbox,
  FormControlLabel
} from '@mui/material';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import WarningAmberIcon from '@mui/icons-material/WarningAmber';

const style = {
  position: 'absolute' as const,
  top: '50%',
  left: '50%',
  transform: 'translate(-50%, -50%)',
  width: 500,
  bgcolor: 'background.paper',
  borderRadius: 2,
  boxShadow: 24,
  p: 4,
};

type Props = {
  open: boolean;
  onClose: () => void;
  wallet: {
    address: string;
    mnemonic: string;
    version: string;
    backedUp: boolean;
  };
};

const BackupModal: React.FC<Props> = ({ open, onClose, wallet }) => {
  const [showMnemonic, setShowMnemonic] = useState(false);
  const [copied, setCopied] = useState(false);
  const [confirmedBackup, setConfirmedBackup] = useState(false);

  console.log("💾 BackupModal render - open:", open, "wallet:", wallet);

  const handleCopy = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <>
      {/* Debug test div */}
      <div style={{
        position: 'fixed',
        top: '50px',
        left: '10px',
        backgroundColor: 'blue',
        color: 'white',
        padding: '10px',
        zIndex: 10000,
        fontSize: '16px',
        fontFamily: 'Arial, sans-serif'
      }}>
        DEBUG: BackupModal rendering - open: {open ? 'YES' : 'NO'}
      </div>

      <Modal
        open={open}
        onClose={(_, reason) => {
          if (reason === 'backdropClick' || reason === 'escapeKeyDown') {
            // Do nothing, prevent closing
            return;
          }
        onClose();
      }}
      disableEscapeKeyDown
    >
      <Box sx={style}>
        <Typography variant="h6" gutterBottom>
          <WarningAmberIcon sx={{ verticalAlign: 'middle', mr: 1, color: 'orange' }} />
          Backup Your Wallet
        </Typography>
        <Typography variant="body2" color="text.secondary" mb={2}>
          This wallet is stored on your computer. If you lose this device or clear its data, your funds and identity may be lost forever.
        </Typography>

        <TextField
          fullWidth
          label="Address"
          value={wallet.address}
          InputProps={{
            endAdornment: (
              <IconButton onClick={() => handleCopy(wallet.address)} size="small">
                <ContentCopyIcon fontSize="small" />
              </IconButton>
            ),
            readOnly: true,
          }}
          margin="normal"
        />

        <TextField
          fullWidth
          label="Wallet Version"
          value={wallet.version}
          InputProps={{
            readOnly: true,
          }}
          margin="normal"
        />

        <Collapse in={showMnemonic}>
          <TextField
            fullWidth
            label="Seed Phrase (Mnemonic)"
            value={wallet.mnemonic}
            multiline
            rows={3}
            InputProps={{
              endAdornment: (
                <IconButton onClick={() => handleCopy(wallet.mnemonic)} size="small">
                  <ContentCopyIcon fontSize="small" />
                </IconButton>
              ),
              readOnly: true,
            }}
            margin="normal"
            helperText="Write down these 12 words in the exact order shown. Store them in a safe place."
          />
        </Collapse>

        <FormControlLabel
          control={
            <Checkbox
              checked={confirmedBackup}
              onChange={(e) => setConfirmedBackup(e.target.checked)}
              name="confirmBackup"
              color="primary"
            />
          }
          label="I have securely backed up my wallet information."
          sx={{ mt: 2 }}
        />

        <Box mt={2} display="flex" justifyContent="space-between">
          <Button onClick={() => setShowMnemonic((prev) => !prev)} color="warning">
            {showMnemonic ? 'Hide Seed Phrase' : 'Show Seed Phrase'}
          </Button>
          <Button
            onClick={async () => {
              try {
                console.log("📝 Marking wallet as backed up...");

                // Set up response listener
                const handleResponse = (event: any) => {
                  if (event.detail.message === 'mark_wallet_backed_up_response') {
                    try {
                      const response = JSON.parse(event.detail.args[0]);
                      console.log("📝 Mark backed up response:", response);

                      if (response.success) {
                        console.log("✅ Wallet successfully marked as backed up");
                      } else {
                        console.error("❌ Failed to mark wallet as backed up:", response.error);
                      }
                    } catch (error) {
                      console.error("💥 Error parsing mark backed up response:", error);
                    }

                    // Remove listener
                    window.removeEventListener('cefMessageResponse', handleResponse);
                  }
                };

                window.addEventListener('cefMessageResponse', handleResponse);

                // Send mark backed up request
                if (window.cefMessage?.send) {
                  window.cefMessage.send('mark_wallet_backed_up', []);
                } else {
                  console.error("❌ cefMessage not available");
                }

                // Cleanup listener after timeout
                setTimeout(() => {
                  window.removeEventListener('cefMessageResponse', handleResponse);
                }, 5000);

              } catch (err) {
                console.error("💥 Error marking wallet as backed up:", err);
              }

              // Close modal regardless of success/failure
              onClose();
            }}
            variant="contained"
            color="primary"
            disabled={!confirmedBackup}
          >
            Done
          </Button>
        </Box>

        {copied && (
          <Typography color="success.main" mt={1} fontSize={14}>
            Copied to clipboard!
          </Typography>
        )}
      </Box>
    </Modal>
    </>
  );
};

export default BackupModal;
