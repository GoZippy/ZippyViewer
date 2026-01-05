ALTER TABLE devices ADD COLUMN group_name TEXT;
ALTER TABLE devices ADD COLUMN tags TEXT; -- JSON array of strings
ALTER TABLE devices ADD COLUMN channel_id TEXT REFERENCES update_channels(id);
