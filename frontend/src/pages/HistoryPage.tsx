import { Box, Container, Typography, Paper } from '@mui/material';
import { HistoryPanel } from '../components/HistoryPanel';

export function HistoryPage() {
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
          <Box sx={{ borderBottom: '1px solid rgba(0, 0, 0, 0.12)', p: 3, bgcolor: '#fafafa' }}>
            <Typography variant="h4" component="h1" sx={{ fontWeight: 500 }}>
              Browsing History
            </Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
              View and manage your browsing history
            </Typography>
          </Box>

          <HistoryPanel />
        </Paper>
      </Container>
    </Box>
  );
}

export default HistoryPage;
