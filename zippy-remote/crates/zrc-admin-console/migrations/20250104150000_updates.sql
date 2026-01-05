-- Update Channels
CREATE TABLE update_channels (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE, -- e.g., 'stable', 'beta', 'nightly'
    description TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Releases
CREATE TABLE releases (
    version TEXT PRIMARY KEY NOT NULL, -- e.g., '1.0.0'
    channel_id TEXT NOT NULL,
    url TEXT NOT NULL, -- S3 URL or similar
    checksum TEXT NOT NULL, -- SHA256 hash
    changelog TEXT,
    published_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(channel_id) REFERENCES update_channels(id) ON DELETE CASCADE
);

-- Seed defaults
INSERT INTO update_channels (id, name, description) VALUES 
('chan_stable', 'Stable', 'Official stable releases'),
('chan_beta', 'Beta', 'Public beta testing releases'),
('chan_dev', 'Development', 'Nightly/Internal builds');
