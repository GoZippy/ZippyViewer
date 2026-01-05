
import { Routes, Route, Navigate } from 'react-router-dom';
import { Layout } from './components/Layout';
import { LoginPage } from './pages/Login';
import { Dashboard } from './pages/Dashboard';
import { DeviceList } from './pages/DeviceList';
import { Infrastructure } from './pages/Infrastructure';
import { Updates } from './pages/Updates';
import { Pairings } from './pages/Pairings';
import { ApiKeys } from './pages/ApiKeys';
import { Audit } from './pages/Audit';
import Settings from './pages/Settings';
import { useAuth } from './auth/AuthProvider';
import { Box, CircularProgress } from '@mui/material';

const ProtectedRoute = ({ children }: { children: React.JSX.Element }) => {
  const { isAuthenticated, isLoading } = useAuth();

  if (isLoading) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100vh' }}>
        <CircularProgress />
      </Box>
    );
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  return children;
};

function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route
        path="/"
        element={
          <ProtectedRoute>
            <Layout />
          </ProtectedRoute>
        }
      >
        <Route index element={<Dashboard />} />
        <Route path="devices" element={<DeviceList />} />
        <Route path="pairings" element={<Pairings />} />
        <Route path="infrastructure" element={<Infrastructure />} />
        <Route path="updates" element={<Updates />} />
        <Route path="api-keys" element={<ApiKeys />} />
        <Route path="audit" element={<Audit />} />
        <Route path="settings" element={<Settings />} />
      </Route>
    </Routes>
  );
}

export default App;
