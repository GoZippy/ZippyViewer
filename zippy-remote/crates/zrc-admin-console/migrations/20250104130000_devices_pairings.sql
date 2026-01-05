-- Devices table
CREATE TABLE devices (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    version TEXT,
    last_seen DATETIME,
    status TEXT NOT NULL DEFAULT 'offline',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Pairings table
CREATE TABLE pairings (
    id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL,
    user_id TEXT NOT NULL, -- The operator who owns the pairing
    status TEXT NOT NULL CHECK (status IN ('active', 'revoked', 'expired')),
    permissions TEXT NOT NULL, -- JSON or comma-separated list
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME,
    FOREIGN KEY(device_id) REFERENCES devices(id) ON DELETE CASCADE,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);
