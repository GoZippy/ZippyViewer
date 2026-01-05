import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import {
    Box, Typography, Paper, Button, TextField, Alert, CircularProgress,
    Dialog, DialogTitle, DialogContent, DialogActions
} from '@mui/material';
import { apiClient } from '../api/client';

const Settings = () => {
    const [setupOpen, setSetupOpen] = useState(false);
    const [totpSecret, setTotpSecret] = useState<{ secret: string, qr_code: string } | null>(null);
    const [verifyCode, setVerifyCode] = useState('');
    const [successMessage, setSuccessMessage] = useState('');
    const [error, setError] = useState('');

    const setupMutation = useMutation({
        mutationFn: async () => {
            const res = await apiClient.post('/auth/totp/setup');
            return res.data;
        },
        onSuccess: (data) => {
            setTotpSecret(data);
            setSetupOpen(true);
        }
    });

    const verifyMutation = useMutation({
        mutationFn: async (code: string) => {
            await apiClient.post('/auth/totp/verify', { code });
        },
        onSuccess: () => {
            setSetupOpen(false);
            setSuccessMessage('Two-Factor Authentication (2FA) has been enabled!');
            setTotpSecret(null);
            setVerifyCode('');
        },
        onError: () => {
            setError('Invalid code. Please try again.');
        }
    });

    const handleVerify = () => {
        verifyMutation.mutate(verifyCode);
    };

    return (
        <Box>
            <Typography variant="h4" gutterBottom>Settings</Typography>

            <Paper sx={{ p: 3, maxWidth: 600 }}>
                <Typography variant="h6" gutterBottom>Security</Typography>

                {successMessage && <Alert severity="success" sx={{ mb: 2 }}>{successMessage}</Alert>}

                <Box sx={{ mt: 2 }}>
                    <Typography variant="body1" gutterBottom>
                        Two-Factor Authentication (2FA) adds an extra layer of security to your account.
                    </Typography>
                    <Button
                        variant="contained"
                        color="primary"
                        onClick={() => setupMutation.mutate()}
                        disabled={setupMutation.isPending}
                    >
                        {setupMutation.isPending ? <CircularProgress size={24} /> : 'Enable 2FA'}
                    </Button>
                </Box>
            </Paper>

            <Dialog open={setupOpen} onClose={() => setSetupOpen(false)}>
                <DialogTitle>Setup 2FA</DialogTitle>
                <DialogContent>
                    {totpSecret && (
                        <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2 }}>
                            <Typography>Scan this QR code with your authenticator app:</Typography>
                            <img src={totpSecret.qr_code} alt="2FA QR Code" style={{ width: 200, height: 200 }} />
                            <Typography variant="caption" color="textSecondary">Secret: {totpSecret.secret}</Typography>

                            <TextField
                                label="Verification Code"
                                fullWidth
                                value={verifyCode}
                                onChange={(e) => setVerifyCode(e.target.value)}
                                error={!!error}
                                helperText={error}
                            />
                        </Box>
                    )}
                </DialogContent>
                <DialogActions>
                    <Button onClick={() => setSetupOpen(false)}>Cancel</Button>
                    <Button onClick={handleVerify} variant="contained" disabled={verifyMutation.isPending}>
                        Verify & Enable
                    </Button>
                </DialogActions>
            </Dialog>
        </Box>
    );
};

export default Settings;
