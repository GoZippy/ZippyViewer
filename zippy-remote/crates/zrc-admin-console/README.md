# ZRC Admin Console

The Admin Console provides a web-based interface for managing the Zippy Remote Control infrastructure.

## Components

- **Backend**: Rust (Axum) server handling API requests, authentication, and database operations.
- **Frontend**: React (Vite) application providing the user interface.

## Prerequisites

- Rust (latest stable)
- Node.js (v18+) & npm
- SQLite

## Running the Application

### 1. Backend

The backend server listens on port `3000` (default).

```bash
cd crates/zrc-admin-console
# Create .env if not exists
echo "DATABASE_URL=sqlite:zrc.db" > .env
# Run migrations
sqlx migrate run
# Start server
cargo run
```

### 2. Frontend

The frontend development server proxies API requests to the backend.

```bash
cd web
npm install
npm run dev
```

Visit `http://localhost:5173` to access the console.

## Default Credentials

If running for the first time, check the database seeding in `main.rs` or logs for default admin credentials (usually `admin` / `admin`).

## Features

- **Dashboard**: System overview and statistics.
- **Devices**: Manage registered ZRC devices.
- **Pairings**: View and revoke client-device pairings.
- **Infrastructure**: Manage Relays and Directory Nodes.
- **Updates**: Publish new software releases.
- **API Keys**: Manage access for external tools.
- **Audit**: View security logs.
