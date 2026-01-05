import { useQuery } from '@tanstack/react-query';
import {
    Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Paper, Typography, Chip, CircularProgress
} from '@mui/material';
import { apiClient } from '../api/client';

interface Device {
    id: string;
    name: string;
    version: string;
    last_seen: string;
    status: string;
    created_at: string;
}

export const DeviceList = () => {
    const { data, isLoading, error } = useQuery<Device[]>({
        queryKey: ['devices'],
        queryFn: async () => {
            const res = await apiClient.get('/devices');
            return res.data;
        }
    });

    if (isLoading) return <CircularProgress />;
    if (error) return <Typography color="error">Error loading devices</Typography>;

    return (
        <Paper sx={{ width: '100%', overflow: 'hidden' }}>
            <Typography variant="h6" sx={{ p: 2 }}>Device List</Typography>
            <TableContainer sx={{ maxHeight: 600 }}>
                <Table stickyHeader aria-label="sticky table">
                    <TableHead>
                        <TableRow>
                            <TableCell>Name</TableCell>
                            <TableCell>Version</TableCell>
                            <TableCell>Status</TableCell>
                            <TableCell>Last Seen</TableCell>
                            <TableCell>Created At</TableCell>
                        </TableRow>
                    </TableHead>
                    <TableBody>
                        {data?.map((device) => (
                            <TableRow hover role="checkbox" tabIndex={-1} key={device.id}>
                                <TableCell>{device.name}</TableCell>
                                <TableCell>{device.version}</TableCell>
                                <TableCell>
                                    <Chip
                                        label={device.status}
                                        color={device.status === 'online' ? 'success' : 'default'}
                                        size="small"
                                    />
                                </TableCell>
                                <TableCell>{new Date(device.last_seen).toLocaleString()}</TableCell>
                                <TableCell>{new Date(device.created_at).toLocaleDateString()}</TableCell>
                            </TableRow>
                        ))}
                    </TableBody>
                </Table>
            </TableContainer>
        </Paper>
    );
};
