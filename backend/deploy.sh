#!/bin/bash

# Frame Email Client - Production Deployment Script

set -e

echo "🚀 Starting Frame Email Client deployment..."

# Check if running as root
if [[ $EUID -eq 0 ]]; then
   echo "❌ This script should not be run as root for security reasons"
   exit 1
fi

# Configuration
APP_DIR="/opt/frame-email"
SERVICE_NAME="frame-email"
USER="frame-email"

# Create application user if it doesn't exist
if ! id "$USER" &>/dev/null; then
    echo "📝 Creating application user: $USER"
    sudo useradd -r -s /bin/false -d $APP_DIR $USER
fi

# Create application directory
echo "📁 Creating application directory: $APP_DIR"
sudo mkdir -p $APP_DIR
sudo chown $USER:$USER $APP_DIR

# Build the application
echo "🔨 Building Frame Email Client..."
cargo build --release

# Copy files
echo "📋 Copying application files..."
sudo cp target/release/email-client-backend $APP_DIR/
sudo cp -r migrations $APP_DIR/
sudo cp -r frontend $APP_DIR/
sudo cp .env $APP_DIR/
sudo chown -R $USER:$USER $APP_DIR
sudo chmod +x $APP_DIR/email-client-backend

# Create systemd service
echo "⚙️ Creating systemd service..."
sudo tee /etc/systemd/system/$SERVICE_NAME.service > /dev/null <<EOF
[Unit]
Description=Frame Email Client
After=network.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$APP_DIR
EnvironmentFile=$APP_DIR/.env
ExecStart=$APP_DIR/email-client-backend
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# Reload systemd and enable service
echo "🔄 Enabling and starting service..."
sudo systemctl daemon-reload
sudo systemctl enable $SERVICE_NAME
sudo systemctl restart $SERVICE_NAME

# Wait for service to start
echo "⏳ Waiting for service to start..."
sleep 5

# Check service status
if sudo systemctl is-active --quiet $SERVICE_NAME; then
    echo "✅ Frame Email Client deployed successfully!"
    echo "📊 Service status:"
    sudo systemctl status $SERVICE_NAME --no-pager -l
    echo ""
    echo "🌐 Application should be available at: http://localhost:8080"
    echo "🏥 Health check: http://localhost:8080/health"
    echo ""
    echo "📝 To view logs: sudo journalctl -u $SERVICE_NAME -f"
    echo "🔄 To restart: sudo systemctl restart $SERVICE_NAME"
    echo "🛑 To stop: sudo systemctl stop $SERVICE_NAME"
else
    echo "❌ Service failed to start. Check logs:"
    sudo journalctl -u $SERVICE_NAME --no-pager -l
    exit 1
fi

# Test health endpoint
echo "🧪 Testing health endpoint..."
if curl -f http://localhost:8080/health > /dev/null 2>&1; then
    echo "✅ Health check passed!"
else
    echo "⚠️ Health check failed - service may still be starting"
fi

echo "🎉 Deployment complete!"