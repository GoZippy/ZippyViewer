import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
    Box, Typography, Paper, Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
    IconButton, Tooltip, Chip, CircularProgress
} from '@mui/material';
import { Delete as DeleteIcon } from '@mui/icons-material';
import { apiClient } from '../api/client';

interface Pairing {
    id: string;
    device_id: string;
    user_id: string;
    status: string;
    created_at: string;
    expires_at?: string;
    device_name?: string; // If backend joins it, otherwise we might just show ID
}

export const Pairings = () => {
    const queryClient = useQueryClient();
    const { data: pairings, isLoading } = useQuery<Pairing[]>({
        queryKey: ['pairings'],
        queryFn: async () => (await apiClient.get('/pairings')).data
    });

    const revokeMutation = useMutation({
        mutationFn: async (id: string) => await apiClient.delete(`/pairings/${id}`),
        onSuccess: () => queryClient.invalidateQueries({ queryKey: ['pairings'] })
    });

    if (isLoading) return <CircularProgress />;

    return (
        <Box>
            <Typography variant="h4" gutterBottom>Pairings</Typography>
            <Paper sx={{ width: '100%', overflow: 'hidden', p: 2 }}>
                <TableContainer>
                    <Table stickyHeader>
                        <TableHead>
                            <TableRow>
                                <TableCell>ID</TableCell>
                                <TableCell>Device ID</TableCell>
                                <TableCell>User ID</TableCell>
                                <TableCell>Status</TableCell>
                                <TableCell>Created At</TableCell>
                                <TableCell>Expires At</TableCell>
                                <TableCell>Actions</TableCell>
                            </TableRow>
                        </TableHead>
                        <TableBody>
                            {pairings?.map((pairing) => (
                                <TableRow key={pairing.id}>
                                    <TableCell>{pairing.id.substring(0, 8)}...</TableCell>
                                    <TableCell>{pairing.device_id}</TableCell>
                                    <TableCell>{pairing.user_id}</TableCell>
                                    <TableCell>
                                        <Chip label={pairing.status} color={pairing.status === 'active' ? 'success' : 'default'} size="small" />
                                    </TableCell>
                                    <TableCell>{new Date(pairing.created_at).toLocaleDateString()}</TableCell>
                                    <TableCell>{pairing.expires_at ? new Date(pairing.expires_at).toLocaleDateString() : 'Never'}</TableCell>
                                    <TableCell>
                                        <Tooltip title="Revoke Pairing">
                                            <IconButton color="error" onClick={() => {
                                                if (confirm('Are you sure you want to revoke this pairing?')) {
                                                    revokeMutation.mutate(pairing.id);
                                                }
                                            }}>
                                                <DeleteIcon />
                                            </IconButton>
                                        </Tooltip>
                                    </TableCell>
                                </TableRow>
                            ))}
                        </TableBody>
                    </Table>
                </TableContainer>
            </Paper>
        </Box>
    );
};
