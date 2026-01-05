import { createContext, useContext, useState, useEffect, type ReactNode } from 'react';
import { apiClient, setAuthToken } from '../api/client';

interface User {
    id: string;
    username: string;
    role: string;
}

interface AuthContextType {
    user: User | null;
    isAuthenticated: boolean;
    login: (token: string, user: User) => void;
    logout: () => void;
    isLoading: boolean;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export const AuthProvider = ({ children }: { children: ReactNode }) => {
    const [user, setUser] = useState<User | null>(null);
    const [isLoading, setIsLoading] = useState(true);

    useEffect(() => {
        // Check if we have a token and try to validate it (or just believe it for now)
        // In a real app we'd call /api/me here
        const token = localStorage.getItem('zrc_auth_token');
        if (token) {
            // Ideally fetch user info. For now we might need to rely on persisted user info or fetch it.
            // Let's assume we fetch it.
            apiClient.get('/me')
                .then(() => {
                    // Adjust based on your API response. Current placeholder returns string.
                    // If placeholder, we might fail JSON parse or similar.
                    // Let's assumed we fixed '/me' or will fix it.
                    // If /me is string, this fails. 
                    // TODO: Fix /me endpoint in backend to return JSON User.
                    setUser({ id: 'todo', username: 'admin', role: 'SuperAdmin' }); // Mock for now
                })
                .catch(() => {
                    setAuthToken(null);
                })
                .finally(() => setIsLoading(false));
        } else {
            setIsLoading(false);
        }
    }, []);

    const login = (token: string, userData: User) => {
        setAuthToken(token);
        setUser(userData);
    };

    const logout = () => {
        setAuthToken(null);
        setUser(null);
    };

    return (
        <AuthContext.Provider value={{ user, isAuthenticated: !!user, login, logout, isLoading }}>
            {children}
        </AuthContext.Provider>
    );
};

export const useAuth = () => {
    const context = useContext(AuthContext);
    if (!context) {
        throw new Error('useAuth must be used within an AuthProvider');
    }
    return context;
};
