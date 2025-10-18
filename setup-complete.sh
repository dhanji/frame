#!/bin/bash

# Frame Email Client - Complete Setup Script

set -e

echo "🚀 Setting up Frame Email Client..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check for required tools
check_requirement() {
    if ! command -v $1 &> /dev/null; then
        echo -e "${RED}❌ $1 is not installed${NC}"
        echo "Please install $1 and try again"
        exit 1
    else
        echo -e "${GREEN}✓ $1 found${NC}"
    fi
}

echo "Checking requirements..."
check_requirement "cargo"
check_requirement "npm"
check_requirement "sqlite3"

# Backend Setup
echo -e "\n${YELLOW}Setting up Backend...${NC}"
cd backend

# Create .env file if it doesn't exist
if [ ! -f .env ]; then
    echo "Creating .env file..."
    cat > .env << EOF
DATABASE_URL=sqlite:email_client.db
JWT_SECRET=$(openssl rand -hex 32)
ENCRYPTION_KEY=$(openssl rand -hex 32)
RUST_LOG=info
SERVER_HOST=127.0.0.1
SERVER_PORT=8080
EOF
    echo -e "${GREEN}✓ .env file created${NC}"
fi

# Initialize database
echo "Initializing database..."
sqlite3 email_client.db < migrations/001_initial_schema.sql
echo -e "${GREEN}✓ Database initialized${NC}"

# Build backend
echo "Building backend..."
cargo build --release
echo -e "${GREEN}✓ Backend built${NC}"

cd ..

# Frontend Setup
echo -e "\n${YELLOW}Setting up Frontend...${NC}"
cd frontend

# Install dependencies
echo "Installing frontend dependencies..."
npm install
echo -e "${GREEN}✓ Frontend dependencies installed${NC}"

# Create environment file
if [ ! -f .env ]; then
    echo "Creating frontend .env file..."
    cat > .env << EOF
VITE_API_URL=http://localhost:8080/api
VITE_WS_URL=ws://localhost:8080/ws
EOF
    echo -e "${GREEN}✓ Frontend .env file created${NC}"
fi

# Build frontend
echo "Building frontend..."
npm run build
echo -e "${GREEN}✓ Frontend built${NC}"

cd ..

# Create run script
echo -e "\n${YELLOW}Creating run scripts...${NC}"

cat > run-backend.sh << 'EOF'
#!/bin/bash
cd backend
cargo run --release
EOF
chmod +x run-backend.sh

cat > run-frontend.sh << 'EOF'
#!/bin/bash
cd frontend
npm run dev
EOF
chmod +x run-frontend.sh

cat > run-all.sh << 'EOF'
#!/bin/bash
# Run both backend and frontend
trap 'kill 0' EXIT

echo "Starting backend..."
./run-backend.sh &
BACKEND_PID=$!

echo "Waiting for backend to start..."
sleep 3

echo "Starting frontend..."
./run-frontend.sh &
FRONTEND_PID=$!

echo "Frame Email Client is running!"
echo "Backend: http://localhost:8080"
echo "Frontend: http://localhost:5173"
echo "Press Ctrl+C to stop"

wait
EOF
chmod +x run-all.sh

echo -e "${GREEN}✓ Run scripts created${NC}"

# Create test user
echo -e "\n${YELLOW}Creating test user...${NC}"
sqlite3 backend/email_client.db << EOF
INSERT OR IGNORE INTO users (email, username, password_hash, imap_host, smtp_host)
VALUES (
    'test@example.com',
    'testuser',
    '\$2b\$12\$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY/MrLJJwJHYJhC',  -- password: test123
    'imap.gmail.com',
    'smtp.gmail.com'
);
EOF
echo -e "${GREEN}✓ Test user created (username: testuser, password: test123)${NC}"

echo -e "\n${GREEN}🎉 Setup complete!${NC}"
echo -e "\nTo run the application:"
echo -e "  ${YELLOW}./run-all.sh${NC}     - Run both backend and frontend"
echo -e "  ${YELLOW}./run-backend.sh${NC} - Run backend only"
echo -e "  ${YELLOW}./run-frontend.sh${NC} - Run frontend only"
echo -e "\nAccess the application at: ${GREEN}http://localhost:5173${NC}"
echo -e "\n${YELLOW}Note:${NC} You'll need to configure your email settings in the application."
echo -e "For Gmail, you may need to:"
echo -e "  1. Enable 2-factor authentication"
echo -e "  2. Generate an app-specific password"
echo -e "  3. Use the app password instead of your regular password"