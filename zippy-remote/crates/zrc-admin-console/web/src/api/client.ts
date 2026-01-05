import axios from 'axios';

const API_URL = import.meta.env.VITE_API_URL || '/api';

export const apiClient = axios.create({
  baseURL: API_URL,
  headers: {
    'Content-Type': 'application/json',
  },
});

export const setAuthToken = (token: string | null) => {
  if (token) {
    apiClient.defaults.headers.common['Authorization'] = `Bearer ${token}`;
    localStorage.setItem('zrc_auth_token', token);
  } else {
    delete apiClient.defaults.headers.common['Authorization'];
    localStorage.removeItem('zrc_auth_token');
  }
};

// Initialize from local storage
const storedToken = localStorage.getItem('zrc_auth_token');
if (storedToken) {
  setAuthToken(storedToken);
}

apiClient.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401) {
      // Auto logout on 401?
      // We might need an event emitter or callback to AuthProvider to clear state
      // For now, just clear token
      setAuthToken(null);
      window.location.href = '/login';
    }
    return Promise.reject(error);
  }
);
