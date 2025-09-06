# Lobby Server

The lobby server is an HTTP service that provides REST API endpoints for room management. It runs on port 3001 and handles requests proxied through Caddy/nginx at `/hook/api/*`.

## Endpoints

- `GET /api/health` - Health check endpoint
- `GET /api/rooms` - List all rooms (supports `?status=waiting` query parameter)
- `POST /api/rooms` - Create a new room
- `POST /api/rooms/:room_id/join` - Join an existing room

## Usage

### Running the server

From the repository root:

```bash
cd lobby-server
cargo run
```

The server will start on port 3001 and display available endpoints.

### API Examples

#### List rooms
```bash
curl http://localhost:3001/api/rooms
```

#### Create a room
```bash
curl -X POST http://localhost:3001/api/rooms \
  -H "Content-Type: application/json" \
  -d '{"name": "My Room", "max_players": 4}'
```

#### Join a room
```bash
curl -X POST http://localhost:3001/api/rooms/room_001/join \
  -H "Content-Type: application/json" \
  -d '{"player_name": "PlayerName"}'
```

## Problem Solved

This server resolves the "invalid json value <!DOCTYPE" error that occurs when clicking "join room". Previously, the client was making requests to `/hook/api/rooms` but no server was running to handle these requests, causing HTML error pages to be returned instead of JSON.

Now with this lobby server running on port 3001, the proxy configuration can successfully route `/hook/*` requests to the lobby service, and proper JSON responses are returned.