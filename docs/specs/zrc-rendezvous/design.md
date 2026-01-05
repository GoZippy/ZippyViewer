# zrc-rendezvous Design

## Trust model
- Rendezvous is NOT trusted for confidentiality or integrity.
- Endpoints authenticate via envelope signature + pinned keys.

## Storage
- In-memory queue (MVP)
- Optional persistent backend (Redis) later.

## API behavior
- POST returns 202 if enqueued, 413 if too large, 429 if rate limited.
- GET returns 200 with raw bytes, or 204 if none.

## Deployment
- Docker optional; native binary preferred.
- TLS termination at reverse proxy optional.
