import { useQuery } from '@tanstack/react-query';
import {
    Box, Typography, Paper, Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
    TextField, CircularProgress, Button
} from '@mui/material';
import { Download as DownloadIcon } from '@mui/icons-material';
import { apiClient } from '../api/client';

interface AuditLog {
    id: string;
    user_id?: string;
    action: string;
    resource_type: string;
    resource_id?: string;
    details?: string;
    ip_address?: string;
    created_at: string;
}

export const Audit = () => {
    const { data: logs, isLoading } = useQuery<AuditLog[]>({
        queryKey: ['audit-logs'],
        queryFn: async () => (await apiClient.get('/audit-logs')).data
    });

    if (isLoading) return <CircularProgress />;

    const handleExport = () => {
        window.open(`${apiClient.defaults.baseURL}/audit-logs/export`, '_blank');
    };

    return (
        <Box>
            <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 3 }}>
                <Typography variant="h4">Audit Logs</Typography>
                <Button variant="outlined" startIcon={<DownloadIcon />} onClick={handleExport}>
                    Export CSV
                </Button>
            </Box>

            <TextField
                placeholder="Search logs..."
                fullWidth
                sx={{ mb: 2 }}
            />
            <Paper sx={{ width: '100%', overflow: 'hidden', p: 2 }}>
                <TableContainer sx={{ maxHeight: '80vh' }}>
                    <Table stickyHeader size="small">
                        <TableHead>
                            <TableRow>
                                <TableCell>Time</TableCell>
                                <TableCell>User</TableCell>
                                <TableCell>Action</TableCell>
                                <TableCell>Resource</TableCell>
                                <TableCell>Details</TableCell>
                                <TableCell>IP</TableCell>
                            </TableRow>
                        </TableHead>
                        <TableBody>
                            {logs?.map((log) => (
                                <TableRow key={log.id}>
                                    <TableCell>{new Date(log.created_at).toLocaleString()}</TableCell>
                                    <TableCell>{log.user_id || 'System'}</TableCell>
                                    <TableCell>{log.action}</TableCell>
                                    <TableCell>{log.resource_type} {log.resource_id ? `(${log.resource_id})` : ''}</TableCell>
                                    <TableCell>{log.details || '-'}</TableCell>
                                    <TableCell>{log.ip_address || '-'}</TableCell>
                                </TableRow>
                            ))}
                        </TableBody>
                    </Table>
                </TableContainer>
            </Paper>
        </Box>
    );
};
