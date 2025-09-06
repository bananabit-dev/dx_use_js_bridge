# Use the official Rust image as the base
FROM rust:1.70 as builder

# Set the working directory
WORKDIR /app

# Copy the lobby server source
COPY lobby-server/ .

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install ca-certificates for HTTPS requests
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/lobby-server /usr/local/bin/lobby-server

# Expose port 3001
EXPOSE 3001

# Run the server
CMD ["lobby-server"]