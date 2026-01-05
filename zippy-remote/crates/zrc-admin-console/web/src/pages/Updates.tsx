import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
    Box, Typography, Paper, Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
    Button, TextField, Dialog, DialogTitle, DialogContent, DialogActions, Grid,
    Select, MenuItem, FormControl, InputLabel
} from '@mui/material';
import { apiClient } from '../api/client';
import { useFormik } from 'formik';
import * as yup from 'yup';

interface Channel {
    id: string;
    name: string;
    latest_release_id?: string;
}

interface Release {
    id: string;
    version: string;
    channel_id: string;
    platform: string;
    url: string;
    created_at: string;
}

const PublishReleaseDialog = ({ open, onClose, channels }: { open: boolean, onClose: () => void, channels: Channel[] }) => {
    const queryClient = useQueryClient();
    const formik = useFormik({
        initialValues: { version: '', channel_id: '', platform: '', url: '', signature: '' },
        validationSchema: yup.object({
            version: yup.string().required(),
            channel_id: yup.string().required(),
            platform: yup.string().required(),
            url: yup.string().required(),
        }),
        onSubmit: async (values) => {
            await apiClient.post('/updates/releases', values);
            queryClient.invalidateQueries({ queryKey: ['releases'] });
            onClose();
            formik.resetForm();
        }
    });

    return (
        <Dialog open={open} onClose={onClose}>
            <DialogTitle>Publish Release</DialogTitle>
            <DialogContent>
                <Box component="form" onSubmit={formik.handleSubmit} sx={{ mt: 1, display: 'flex', flexDirection: 'column', gap: 2 }}>
                    <TextField
                        label="Version" name="version"
                        value={formik.values.version} onChange={formik.handleChange}
                        error={formik.touched.version && Boolean(formik.errors.version)}
                    />
                    <FormControl fullWidth>
                        <InputLabel>Channel</InputLabel>
                        <Select
                            name="channel_id"
                            label="Channel"
                            value={formik.values.channel_id}
                            onChange={formik.handleChange}
                        >
                            {channels.map(c => <MenuItem key={c.id} value={c.id}>{c.name}</MenuItem>)}
                        </Select>
                    </FormControl>
                    <TextField label="Platform" name="platform" value={formik.values.platform} onChange={formik.handleChange} />
                    <TextField label="URL" name="url" value={formik.values.url} onChange={formik.handleChange} />
                    <TextField label="Signature" name="signature" value={formik.values.signature} onChange={formik.handleChange} multiline rows={3} />
                </Box>
            </DialogContent>
            <DialogActions>
                <Button onClick={onClose}>Cancel</Button>
                <Button onClick={() => formik.handleSubmit()} variant="contained">Publish</Button>
            </DialogActions>
        </Dialog>
    );
};

export const Updates = () => {
    const [publishOpen, setPublishOpen] = useState(false);
    const { data: channels } = useQuery<Channel[]>({
        queryKey: ['channels'],
        queryFn: async () => (await apiClient.get('/updates/channels')).data
    });
    const { data: releases } = useQuery<Release[]>({
        queryKey: ['releases'],
        queryFn: async () => (await apiClient.get('/updates/releases')).data
    });

    return (
        <Box>
            <Grid container spacing={4}>
                <Grid size={{ xs: 12 }}>
                    <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 2 }}>
                        <Typography variant="h4">Update Management</Typography>
                        <Button variant="contained" onClick={() => setPublishOpen(true)}>Publish Release</Button>
                    </Box>
                </Grid>

                <Grid size={{ xs: 12, md: 4 }}>
                    <Paper sx={{ p: 2 }}>
                        <Typography variant="h6" gutterBottom>Channels</Typography>
                        <TableContainer>
                            <Table size="small">
                                <TableHead><TableRow><TableCell>Name</TableCell><TableCell>Latest Release</TableCell></TableRow></TableHead>
                                <TableBody>
                                    {channels?.map(c => (
                                        <TableRow key={c.id}>
                                            <TableCell>{c.name}</TableCell>
                                            <TableCell>{c.latest_release_id || 'None'}</TableCell>
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        </TableContainer>
                    </Paper>
                </Grid>

                <Grid size={{ xs: 12, md: 8 }}>
                    <Paper sx={{ p: 2 }}>
                        <Typography variant="h6" gutterBottom>Release History</Typography>
                        <TableContainer>
                            <Table size="small">
                                <TableHead><TableRow><TableCell>Version</TableCell><TableCell>Platform</TableCell><TableCell>Channel</TableCell><TableCell>Date</TableCell></TableRow></TableHead>
                                <TableBody>
                                    {releases?.map(r => (
                                        <TableRow key={r.id}>
                                            <TableCell>{r.version}</TableCell>
                                            <TableCell>{r.platform}</TableCell>
                                            <TableCell>{channels?.find(c => c.id === r.channel_id)?.name || r.channel_id}</TableCell>
                                            <TableCell>{new Date(r.created_at).toLocaleDateString()}</TableCell>
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        </TableContainer>
                    </Paper>
                </Grid>
            </Grid>

            <PublishReleaseDialog open={publishOpen} onClose={() => setPublishOpen(false)} channels={channels || []} />
        </Box>
    );
};
