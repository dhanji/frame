#!/bin/bash

echo "Setting up Frame Email Client..."

# Backend setup
echo "\n=== Setting up Backend ==="
cd email-client/backend

# Create database directory
mkdir -p data

# Create .env file if it doesn't exist
if [ ! -f .env ]; then
    echo "Creating .env file..."
    cat > .env << EOF
DATABASE_URL=sqlite:data/email_client.db
JWT_SECRET=your-secret-key-change-in-production-$(openssl rand -hex 32)
JWT_EXPIRATION=86400
SERVER_HOST=127.0.0.1
SERVER_PORT=8080
ENCRYPTION_KEY=0123456789abcdef0123456789abcdef
EOF
fi

# Build backend
echo "Building backend..."
cargo build --release 2>&1 | head -20

if [ $? -eq 0 ]; then
    echo "✅ Backend built successfully!"
else
    echo "❌ Backend build failed. Checking dependencies..."
    cargo check 2>&1 | head -20
fi

# Frontend setup
echo "\n=== Setting up Frontend ==="
cd ../frontend

# Install dependencies
echo "Installing frontend dependencies..."
npm install 2>&1 | tail -5

if [ $? -eq 0 ]; then
    echo "✅ Frontend dependencies installed!"
else
    echo "❌ Frontend dependency installation failed"
fi

# Check TypeScript compilation
echo "Checking TypeScript compilation..."
npx tsc --noEmit 2>&1 | head -10

echo "\n=== Setup Complete ==="
echo "To run the application:"
echo "1. Backend: cd email-client/backend && cargo run"
echo "2. Frontend: cd email-client/frontend && npm run dev"
echo "3. Open http://localhost:3000 in your browser"