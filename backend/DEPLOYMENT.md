# Frame Email Client - Deployment Guide

## Production Deployment

### Prerequisites
- Rust 1.70+ installed
- SQLite 3.x
- Valid email server credentials (IMAP/SMTP)

### Environment Setup

1. **Copy and configure environment file:**
```bash
cp .env.example .env
```

2. **Edit `.env` file with production values:**
```bash
# Database Configuration
DATABASE_URL=sqlite:email_client.db

# JWT Configuration (CHANGE IN PRODUCTION)
JWT_SECRET=your_secure_jwt_secret_key_here_32_chars_minimum

# Encryption Configuration (CHANGE IN PRODUCTION)
ENCRYPTION_KEY=your_secure_encryption_key_32_bytes_long_here

# Server Configuration
HOST=0.0.0.0
PORT=8080

# Production Mode
DEMO_MODE=false

# Logging
RUST_LOG=info

# CORS (adjust for your domain)
CORS_ALLOWED_ORIGINS=https://yourdomain.com

# Rate Limiting
RATE_LIMIT_REQUESTS_PER_MINUTE=60
```

### Build and Run

1. **Build for production:**
```bash
cargo build --release
```

2. **Run the server:**
```bash
./target/release/email-client-backend
```

### Docker Deployment

1. **Create Dockerfile:**
```dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/email-client-backend .
COPY --from=builder /app/migrations ./migrations
COPY frontend ./frontend

EXPOSE 8080

CMD ["./email-client-backend"]
```

2. **Build and run Docker container:**
```bash
docker build -t frame-email-client .
docker run -p 8080:8080 --env-file .env frame-email-client
```

### Nginx Reverse Proxy

```nginx
server {
    listen 80;
    server_name yourdomain.com;
    
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
    
    location /ws {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
    }
}
```

### SSL/TLS Setup

```bash
# Install certbot
sudo apt install certbot python3-certbot-nginx

# Get SSL certificate
sudo certbot --nginx -d yourdomain.com
```

### Systemd Service

Create `/etc/systemd/system/frame-email.service`:

```ini
[Unit]
Description=Frame Email Client
After=network.target

[Service]
Type=simple
User=www-data
WorkingDirectory=/opt/frame-email
EnvironmentFile=/opt/frame-email/.env
ExecStart=/opt/frame-email/email-client-backend
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable frame-email
sudo systemctl start frame-email
```

### Monitoring

1. **Health Check Endpoint:**
```bash
curl http://localhost:8080/health
```

2. **Log Monitoring:**
```bash
journalctl -u frame-email -f
```

### Backup Strategy

1. **Database Backup:**
```bash
#!/bin/bash
cp email_client.db "email_client_backup_$(date +%Y%m%d_%H%M%S).db"
```

2. **Automated Backup (crontab):**
```bash
0 2 * * * /opt/frame-email/backup.sh
```

### Security Checklist

- [ ] Change default JWT_SECRET and ENCRYPTION_KEY
- [ ] Set DEMO_MODE=false in production
- [ ] Configure proper CORS origins
- [ ] Enable HTTPS with valid SSL certificate
- [ ] Set up firewall rules
- [ ] Regular security updates
- [ ] Monitor logs for suspicious activity
- [ ] Backup database regularly

### Troubleshooting

1. **Server won't start:**
   - Check `.env` file exists and has correct values
   - Verify database permissions
   - Check port availability

2. **Database errors:**
   - Ensure SQLite is installed
   - Check file permissions
   - Verify migrations ran successfully

3. **Email connection issues:**
   - Verify IMAP/SMTP credentials
   - Check firewall rules
   - Test email server connectivity

4. **WebSocket issues:**
   - Check proxy configuration
   - Verify WebSocket upgrade headers
   - Test connection directly

### Performance Tuning

1. **Database optimization:**
   - Regular VACUUM operations
   - Monitor query performance
   - Consider connection pooling adjustments

2. **Server optimization:**
   - Adjust worker threads
   - Monitor memory usage
   - Configure appropriate timeouts

3. **Caching:**
   - Enable HTTP caching headers
   - Consider Redis for session storage
   - Implement email caching strategies