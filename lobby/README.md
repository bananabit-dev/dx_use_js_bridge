# Lobby HTTP Service

This directory contains the HTTP lobby service that fixes the "invalid json value <!DOCTYPE" error by providing proper JSON endpoints.

## Problem Solved

The client code was making requests to `/hook/api/rooms` expecting JSON responses, but there was no HTTP server running to handle these endpoints. This caused requests to return HTML error pages (hence the `<!DOCTYPE` error), instead of the expected JSON responses.

## Solution

This lobby service provides:

- **GET /api/rooms** - Returns a list of available rooms in JSON format
- **POST /api/rooms** - Create a new room
- **POST /api/rooms/:id/join** - Join an existing room  
- **GET /health** - Health check endpoint

## Running the Service

### Local Development

```bash
cd lobby
cargo run
```

The server will start on `http://localhost:3001` and serve the following endpoints:

- `http://localhost:3001/api/rooms`
- `http://localhost:3001/health`

### Using Docker

```bash
# Build the Docker image
docker build -t lobby-server ./lobby

# Run the container
docker run -p 3001:3001 lobby-server
```

### Using Docker Compose

```bash
# Start both lobby service and nginx proxy
docker-compose up

# The proxy will be available on port 80 and route /hook/* to the lobby service
curl http://localhost/hook/api/rooms
```

## API Endpoints

### GET /api/rooms

Returns a list of available rooms:

```json
{
  "rooms": [
    {
      "id": "room_1",
      "name": "Lobby Room 1", 
      "players": ["player_1"],
      "max_players": 4,
      "created_at": "2024-01-01T00:00:00Z"
    }
  ],
  "total": 1
}
```

### POST /api/rooms

Create a new room:

**Request:**
```json
{
  "name": "My New Room",
  "max_players": 6
}
```

**Response:**
```json
{
  "id": "room_1757142086",
  "name": "My New Room",
  "players": [],
  "max_players": 6,
  "created_at": "2025-09-06T07:01:26.548582942+00:00"
}
```

### POST /api/rooms/:id/join

Join an existing room:

**Request:**
```json
{
  "room_id": "room_1", 
  "player_name": "player123"
}
```

**Response:**
```json
{
  "success": true,
  "room_id": "room_1",
  "player_name": "player123", 
  "message": "Successfully joined room"
}
```

## Testing

Test the endpoints manually:

```bash
# Get rooms
curl http://localhost:3001/api/rooms

# Health check  
curl http://localhost:3001/health

# Create room
curl -X POST http://localhost:3001/api/rooms \
  -H "Content-Type: application/json" \
  -d '{"name": "Test Room", "max_players": 4}'

# Join room
curl -X POST http://localhost:3001/api/rooms/room_1/join \
  -H "Content-Type: application/json" \
  -d '{"room_id": "room_1", "player_name": "testplayer"}'
```