import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
    Box, Typography, Paper, Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
    Button, IconButton, Tooltip, CircularProgress, Dialog, DialogTitle, DialogContent,
    DialogActions, TextField, Alert
} from '@mui/material';
import { Delete as DeleteIcon, ContentCopy as CopyIcon } from '@mui/icons-material';
import { apiClient } from '../api/client';
import { useFormik } from 'formik';
import * as yup from 'yup';

interface ApiKey {
    id: string;
    prefix: string;
    name: string;
    created_at: string;
    expires_at?: string;
    permissions: string; // JSON string
}

const CreateKeyDialog = ({ open, onClose }: { open: boolean, onClose: () => void }) => {
    const queryClient = useQueryClient();
    const [createdKey, setCreatedKey] = useState<string | null>(null);

    const formik = useFormik({
        initialValues: { name: '', permissions: '["*"]', expires_in_days: '' },
        validationSchema: yup.object({
            name: yup.string().required(),
            permissions: yup.string().required(), // Simple JSON validation could be added
        }),
        onSubmit: async (values) => {
            const res = await apiClient.post('/api-keys', values);
            // Response: { key: "zrc_..." } along with key object
            setCreatedKey(res.data.key);
            queryClient.invalidateQueries({ queryKey: ['api-keys'] });
        }
    });

    const handleClose = () => {
        setCreatedKey(null);
        formik.resetForm();
        onClose();
    };

    return (
        <Dialog open={open} onClose={handleClose}>
            <DialogTitle>Create API Key</DialogTitle>
            <DialogContent>
                {createdKey ? (
                    <Box sx={{ mt: 2 }}>
                        <Alert severity="success">API Key Created! Copy it now, you won't see it again.</Alert>
                        <Box sx={{ display: 'flex', alignItems: 'center', mt: 2, p: 2, bgcolor: 'grey.100', borderRadius: 1 }}>
                            <Typography component="pre" sx={{ flexGrow: 1, m: 0 }}>{createdKey}</Typography>
                            <IconButton onClick={() => navigator.clipboard.writeText(createdKey)}>
                                <CopyIcon />
                            </IconButton>
                        </Box>
                    </Box>
                ) : (
                    <Box component="form" onSubmit={formik.handleSubmit} sx={{ mt: 1 }}>
                        <TextField
                            fullWidth margin="normal" label="Name" name="name"
                            value={formik.values.name} onChange={formik.handleChange}
                            error={formik.touched.name && Boolean(formik.errors.name)}
                        />
                        <TextField
                            fullWidth margin="normal" label="Permissions (JSON)" name="permissions"
                            value={formik.values.permissions} onChange={formik.handleChange}
                            helperText='Example: ["ViewDevices", "ManageSystem"] or ["*"]'
                        />
                        <TextField
                            fullWidth margin="normal" label="Expires in (Days) - Optional" name="expires_in_days"
                            type="number"
                            value={formik.values.expires_in_days} onChange={formik.handleChange}
                        />
                    </Box>
                )}
            </DialogContent>
            <DialogActions>
                {!createdKey && <Button onClick={handleClose}>Cancel</Button>}
                {!createdKey && <Button onClick={() => formik.handleSubmit()} variant="contained">Create</Button>}
                {createdKey && <Button onClick={handleClose} variant="contained">Done</Button>}
            </DialogActions>
        </Dialog>
    );
};

export const ApiKeys = () => {
    const queryClient = useQueryClient();
    const [createOpen, setCreateOpen] = useState(false);
    const { data: keys, isLoading } = useQuery<ApiKey[]>({
        queryKey: ['api-keys'],
        queryFn: async () => (await apiClient.get('/api-keys')).data
    });

    const revokeMutation = useMutation({
        mutationFn: async (id: string) => await apiClient.delete(`/api-keys/${id}`),
        onSuccess: () => queryClient.invalidateQueries({ queryKey: ['api-keys'] })
    });

    if (isLoading) return <CircularProgress />;

    return (
        <Box>
            <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 2 }}>
                <Typography variant="h4">API Keys</Typography>
                <Button variant="contained" onClick={() => setCreateOpen(true)}>Create Key</Button>
            </Box>
            <Paper sx={{ width: '100%', overflow: 'hidden', p: 2 }}>
                <TableContainer>
                    <Table stickyHeader>
                        <TableHead>
                            <TableRow>
                                <TableCell>Name</TableCell>
                                <TableCell>Prefix</TableCell>
                                <TableCell>Permissions</TableCell>
                                <TableCell>Created At</TableCell>
                                <TableCell>Expires At</TableCell>
                                <TableCell>Actions</TableCell>
                            </TableRow>
                        </TableHead>
                        <TableBody>
                            {keys?.map((key) => (
                                <TableRow key={key.id}>
                                    <TableCell>{key.name}</TableCell>
                                    <TableCell>{key.prefix}...</TableCell>
                                    <TableCell>
                                        <Tooltip title={key.permissions}>
                                            <Typography variant="body2" noWrap sx={{ maxWidth: 150 }}>
                                                {key.permissions}
                                            </Typography>
                                        </Tooltip>
                                    </TableCell>
                                    <TableCell>{new Date(key.created_at).toLocaleDateString()}</TableCell>
                                    <TableCell>{key.expires_at ? new Date(key.expires_at).toLocaleDateString() : 'Never'}</TableCell>
                                    <TableCell>
                                        <Tooltip title="Revoke Key">
                                            <IconButton color="error" onClick={() => {
                                                if (confirm('Revoke this API Key?')) {
                                                    revokeMutation.mutate(key.id);
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
            <CreateKeyDialog open={createOpen} onClose={() => setCreateOpen(false)} />
        </Box>
    );
};
