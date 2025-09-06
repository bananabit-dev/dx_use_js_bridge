#!/bin/bash

echo "Testing lobby server endpoints..."

# Start the server in the background
cd lobby
cargo run &
SERVER_PID=$!

# Wait for the server to start
sleep 5

echo "Testing /api/rooms endpoint..."
curl -s http://localhost:3001/api/rooms | jq '.' || echo "Failed to get rooms"

echo ""
echo "Testing /health endpoint..."
curl -s http://localhost:3001/health | jq '.' || echo "Failed to get health"

echo ""
echo "Testing room creation..."
curl -s -X POST http://localhost:3001/api/rooms \
  -H "Content-Type: application/json" \
  -d '{"name": "Test Room", "max_players": 4}' | jq '.' || echo "Failed to create room"

# Clean up
kill $SERVER_PID 2>/dev/null
echo ""
echo "Test completed!"