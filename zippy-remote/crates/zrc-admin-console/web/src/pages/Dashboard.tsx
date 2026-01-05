import React from 'react';
import { useQuery } from '@tanstack/react-query';
import { Paper, Typography, Box, CircularProgress, Grid } from '@mui/material';
import { apiClient } from '../api/client';
import {
    People as PeopleIcon,
    Devices as DevicesIcon,
    Link as LinkIcon,
    Dns as DnsIcon,
} from '@mui/icons-material';

interface DashboardStats {
    total_users: number;
    total_devices: number;
    active_pairings: number;
    total_relays: number;
}

const StatCard = ({ title, value, icon, color }: { title: string, value: number, icon: React.ReactNode, color: string }) => (
    <Paper sx={{ p: 2, display: 'flex', flexDirection: 'column', height: 140, position: 'relative', overflow: 'hidden' }}>
        <Box sx={{ position: 'absolute', top: -10, right: -10, opacity: 0.1, transform: 'scale(2.5)', color: color }}>
            {icon}
        </Box>
        <Typography component="h2" variant="h6" color="text.secondary" gutterBottom>
            {title}
        </Typography>
        <Typography component="p" variant="h3">
            {value}
        </Typography>
    </Paper>
);

export const Dashboard = () => {
    const { data, isLoading, error } = useQuery<DashboardStats>({
        queryKey: ['dashboardStats'],
        queryFn: async () => {
            const res = await apiClient.get('/dashboard/stats');
            return res.data;
        },
    });

    if (isLoading) return <CircularProgress />;
    if (error) return <Typography color="error">Error loading stats</Typography>;

    return (
        <Grid container spacing={3}>
            <Grid size={{ xs: 12, md: 3 }}>
                <StatCard title="Total Users" value={data?.total_users || 0} icon={<PeopleIcon />} color="primary.main" />
            </Grid>
            <Grid size={{ xs: 12, md: 3 }}>
                <StatCard title="Total Devices" value={data?.total_devices || 0} icon={<DevicesIcon />} color="secondary.main" />
            </Grid>
            <Grid size={{ xs: 12, md: 3 }}>
                <StatCard title="Active Pairings" value={data?.active_pairings || 0} icon={<LinkIcon />} color="success.main" />
            </Grid>
            <Grid size={{ xs: 12, md: 3 }}>
                <StatCard title="Relays Online" value={data?.total_relays || 0} icon={<DnsIcon />} color="info.main" />
            </Grid>
            {/* Add Charts or Activity Log here later */}
        </Grid>
    );
};
