# Minesweeper Web

A web-based Minesweeper game built with Rust (Axum) backend and vanilla JavaScript frontend.

## Running Locally (HTTP)

```bash
cargo run
```

The server will start on `http://127.0.0.1:3000`

## Running in Production (HTTPS)

Set the following environment variables and run:

```bash
USE_HTTPS=true CERT_PATH=/path/to/your/cert.pem KEY_PATH=/path/to/your/key.pem ./minesweeper-web
```

The server will start on `https://0.0.0.0:443`

## Building for Production

```bash
cargo build --release
```

The binary will be at `target/release/minesweeper-web`

## Environment Variables

- `USE_HTTPS`: Set to "true" to enable HTTPS mode (default: "false")
- `CERT_PATH`: Path to your SSL certificate file (required when USE_HTTPS=true)
- `KEY_PATH`: Path to your SSL private key file (required when USE_HTTPS=true)

## Example Production Setup

```bash
# Build the project
cargo build --release

# Copy to your server along with static files
scp target/release/minesweeper-web your-server:/path/to/app/
scp -r static your-server:/path/to/app/

# On your server, run with HTTPS
USE_HTTPS=true CERT_PATH=/etc/ssl/certs/your-cert.pem KEY_PATH=/etc/ssl/private/your-key.pem ./minesweeper-web
```
