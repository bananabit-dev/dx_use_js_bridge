#!/bin/bash

# Script to run the lobby server for handling /hook/api/* endpoints

echo "Starting lobby server on port 3001..."
echo "This server handles the /hook/api/rooms endpoints to resolve the 'invalid json value <!DOCTYPE' error"
echo ""

cd "$(dirname "$0")/lobby-server"

if [ ! -f "Cargo.toml" ]; then
    echo "Error: lobby-server/Cargo.toml not found. Please run this script from the repository root."
    exit 1
fi

echo "Building and running lobby server..."
cargo run