import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
    Box, Typography, Paper, Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
    Button, TextField, Dialog, DialogTitle, DialogContent, DialogActions, Grid,
    Chip
} from '@mui/material';
import { apiClient } from '../api/client';
import { useFormik } from 'formik';
import * as yup from 'yup';

interface Relay {
    id: string;
    url: string;
    region?: string;
    status: string;
    connected_clients?: number;
}

interface Dirnode {
    id: string;
    url: string;
    region?: string;
    status: string;
    public_key?: string;
}

const AddRelayDialog = ({ open, onClose }: { open: boolean, onClose: () => void }) => {
    const queryClient = useQueryClient();
    const formik = useFormik({
        initialValues: { url: '', region: '' },
        validationSchema: yup.object({
            url: yup.string().required('URL is required'),
            region: yup.string(),
        }),
        onSubmit: async (values) => {
            await apiClient.post('/infrastructure/relays', values);
            queryClient.invalidateQueries({ queryKey: ['relays'] });
            onClose();
            formik.resetForm();
        }
    });

    return (
        <Dialog open={open} onClose={onClose}>
            <DialogTitle>Add Relay</DialogTitle>
            <DialogContent>
                <Box component="form" onSubmit={formik.handleSubmit} sx={{ mt: 1 }}>
                    <TextField
                        fullWidth margin="normal" label="URL" name="url"
                        value={formik.values.url} onChange={formik.handleChange}
                        error={formik.touched.url && Boolean(formik.errors.url)}
                        helperText={formik.touched.url && formik.errors.url}
                    />
                    <TextField
                        fullWidth margin="normal" label="Region" name="region"
                        value={formik.values.region} onChange={formik.handleChange}
                    />
                </Box>
            </DialogContent>
            <DialogActions>
                <Button onClick={onClose}>Cancel</Button>
                <Button onClick={() => formik.handleSubmit()} variant="contained">Add</Button>
            </DialogActions>
        </Dialog>
    );
};

// Similar Dialog for Dirnode could be created...

export const Infrastructure = () => {
    const [addRelayOpen, setAddRelayOpen] = useState(false);
    const { data: relays } = useQuery<Relay[]>({
        queryKey: ['relays'],
        queryFn: async () => (await apiClient.get('/infrastructure/relays')).data
    });
    const { data: dirnodes } = useQuery<Dirnode[]>({
        queryKey: ['dirnodes'],
        queryFn: async () => (await apiClient.get('/infrastructure/dirnodes')).data
    });

    return (
        <Box>
            <Typography variant="h4" gutterBottom>Infrastructure</Typography>

            <Grid container spacing={4}>
                <Grid size={{ xs: 12, md: 6 }}>
                    <Paper sx={{ p: 2 }}>
                        <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 2 }}>
                            <Typography variant="h6">Relays</Typography>
                            <Button variant="contained" size="small" onClick={() => setAddRelayOpen(true)}>Add Relay</Button>
                        </Box>
                        <TableContainer>
                            <Table size="small">
                                <TableHead>
                                    <TableRow><TableCell>URL</TableCell><TableCell>Region</TableCell><TableCell>Status</TableCell></TableRow>
                                </TableHead>
                                <TableBody>
                                    {relays?.map(r => (
                                        <TableRow key={r.id}>
                                            <TableCell>{r.url}</TableCell>
                                            <TableCell>{r.region || '-'}</TableCell>
                                            <TableCell><Chip label={r.status} size="small" color={r.status === 'active' ? 'success' : 'default'} /></TableCell>
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        </TableContainer>
                    </Paper>
                </Grid>

                <Grid size={{ xs: 12, md: 6 }}>
                    <Paper sx={{ p: 2 }}>
                        <Typography variant="h6" gutterBottom>Directory Nodes</Typography>
                        <TableContainer>
                            <Table size="small">
                                <TableHead>
                                    <TableRow><TableCell>URL</TableCell><TableCell>Region</TableCell><TableCell>Status</TableCell></TableRow>
                                </TableHead>
                                <TableBody>
                                    {dirnodes?.map(d => (
                                        <TableRow key={d.id}>
                                            <TableCell>{d.url}</TableCell>
                                            <TableCell>{d.region || '-'}</TableCell>
                                            <TableCell><Chip label={d.status} size="small" color={d.status === 'active' ? 'success' : 'default'} /></TableCell>
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        </TableContainer>
                    </Paper>
                </Grid>
            </Grid>

            <AddRelayDialog open={addRelayOpen} onClose={() => setAddRelayOpen(false)} />
        </Box>
    );
};
