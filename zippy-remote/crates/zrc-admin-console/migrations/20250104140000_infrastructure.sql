-- Relays
CREATE TABLE relays (
    id TEXT PRIMARY KEY NOT NULL,
    url TEXT NOT NULL,
    region TEXT,
    status TEXT NOT NULL DEFAULT 'active', -- 'active', 'maintenance', 'offline'
    capacity INTEGER DEFAULT 0,
    connected_clients INTEGER DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Dirnodes (Directory Nodes)
CREATE TABLE dirnodes (
    id TEXT PRIMARY KEY NOT NULL,
    url TEXT NOT NULL,
    public_key TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    region TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
