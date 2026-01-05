-- API Keys
CREATE TABLE api_keys (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    key_hash TEXT NOT NULL, -- Argon2 hash of the key
    prefix TEXT NOT NULL, -- First 8 chars for identification
    name TEXT NOT NULL,
    permissions TEXT NOT NULL, -- JSON array of permissions
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME,
    last_used_at DATETIME,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_api_keys_user_id ON api_keys(user_id);
