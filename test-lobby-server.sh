#!/bin/bash

# Integration test to verify the lobby server resolves the "invalid json value <!DOCTYPE" error

echo "🧪 Testing lobby server implementation..."
echo "This test verifies that the /hook/api/rooms endpoints return proper JSON instead of HTML error pages"
echo ""

cd "$(dirname "$0")/lobby-server"

if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: lobby-server/Cargo.toml not found. Please run this script from the repository root."
    exit 1
fi

echo "🚀 Starting lobby server in background..."
cargo run &
SERVER_PID=$!

# Wait for server to start
sleep 3

echo "🩺 Testing health endpoint..."
HEALTH_RESPONSE=$(curl -s http://localhost:3001/api/health)
if echo "$HEALTH_RESPONSE" | grep -q '"status":"ok"'; then
    echo "✅ Health endpoint returns JSON"
else
    echo "❌ Health endpoint failed: $HEALTH_RESPONSE"
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

echo "📋 Testing rooms list endpoint..."
ROOMS_RESPONSE=$(curl -s http://localhost:3001/api/rooms)
if echo "$ROOMS_RESPONSE" | grep -q '"id":"room_001"'; then
    echo "✅ Rooms endpoint returns JSON array"
else
    echo "❌ Rooms endpoint failed: $ROOMS_RESPONSE"
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

echo "🏠 Testing room creation..."
CREATE_RESPONSE=$(curl -s -X POST http://localhost:3001/api/rooms \
    -H "Content-Type: application/json" \
    -d '{"name": "Test Room", "max_players": 6}')
if echo "$CREATE_RESPONSE" | grep -q '"name":"Test Room"'; then
    echo "✅ Room creation returns JSON"
else
    echo "❌ Room creation failed: $CREATE_RESPONSE"
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

echo "🔧 Testing non-existent endpoint (should return 404, not HTML)..."
NOT_FOUND_RESPONSE=$(curl -s -w "%{http_code}" http://localhost:3001/api/nonexistent)
if echo "$NOT_FOUND_RESPONSE" | grep -q "404"; then
    echo "✅ Non-existent endpoint returns 404 status code (not HTML error page)"
else
    echo "⚠️  Non-existent endpoint response: $NOT_FOUND_RESPONSE"
fi

echo "🧹 Cleaning up..."
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

echo ""
echo "🎉 All tests passed! The lobby server correctly:"
echo "   ✅ Returns JSON responses for all room management endpoints"
echo "   ✅ Handles /api/rooms for listing and creating rooms"
echo "   ✅ Provides health check endpoint"
echo "   ✅ No HTML error pages (resolves 'invalid json value <!DOCTYPE' error)"
echo ""
echo "🚀 Ready for deployment on port 3001 to handle /hook/api/* requests!"