#!/bin/bash

# Email Client Backend Runner
# This script manages the backend server with options to start, stop, and restart

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKEND_DIR="$SCRIPT_DIR/backend"
PID_FILE="$SCRIPT_DIR/.backend.pid"
LOG_FILE="$SCRIPT_DIR/backend.log"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored messages
print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# Function to check if server is running
is_running() {
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE")
        if ps -p "$PID" > /dev/null 2>&1; then
            return 0
        else
            # PID file exists but process is dead
            rm -f "$PID_FILE"
            return 1
        fi
    fi
    return 1
}

# Function to get the PID
get_pid() {
    if [ -f "$PID_FILE" ]; then
        cat "$PID_FILE"
    fi
}

# Function to kill server by port
kill_by_port() {
    local PORT=${1:-8080}
    print_info "Checking for processes on port $PORT..."
    
    # Find process using lsof
    local PIDS=$(lsof -ti :$PORT 2>/dev/null || true)
    
    if [ -z "$PIDS" ]; then
        print_info "No process found on port $PORT"
        return 0
    fi
    
    for PID in $PIDS; do
        print_warning "Killing process $PID on port $PORT..."
        kill -9 "$PID" 2>/dev/null || true
        print_success "Process $PID killed"
    done
}

# Function to stop the server
stop_server() {
    if is_running; then
        PID=$(get_pid)
        print_info "Stopping backend server (PID: $PID)..."
        
        # Try graceful shutdown first
        kill "$PID" 2>/dev/null || true
        
        # Wait up to 5 seconds for graceful shutdown
        for i in {1..5}; do
            if ! ps -p "$PID" > /dev/null 2>&1; then
                break
            fi
            sleep 1
        done
        
        # Force kill if still running
        if ps -p "$PID" > /dev/null 2>&1; then
            print_warning "Forcing shutdown..."
            kill -9 "$PID" 2>/dev/null || true
        fi
        
        rm -f "$PID_FILE"
        print_success "Backend server stopped"
    else
        print_info "Backend server is not running"
    fi
    
    # Also check and kill any process on port 8080
    kill_by_port 8080
}

# Function to build the backend
build_backend() {
    print_info "Building backend..."
    cd "$BACKEND_DIR"
    
    if cargo build --release 2>&1 | tee -a "$LOG_FILE"; then
        print_success "Backend built successfully"
        return 0
    else
        print_error "Failed to build backend"
        return 1
    fi
}

# Function to start the server
start_server() {
    if is_running; then
        PID=$(get_pid)
        print_warning "Backend server is already running (PID: $PID)"
        print_info "Use '$0 restart' to restart or '$0 stop' to stop"
        return 1
    fi
    
    # Check if port 8080 is already in use
    if lsof -i :8080 > /dev/null 2>&1; then
        print_error "Port 8080 is already in use"
        print_info "Run '$0 kill' to kill the process using port 8080"
        return 1
    fi
    
    print_info "Starting backend server..."
    cd "$BACKEND_DIR"
    
    # Check if binary exists
    if [ ! -f "target/release/server" ]; then
        print_warning "Backend binary not found, building..."
        if ! build_backend; then
            return 1
        fi
    fi
    
    # Start the server in background
    RUST_LOG=info ./target/release/server >> "$LOG_FILE" 2>&1 &
    local PID=$!
    
    # Save PID
    echo "$PID" > "$PID_FILE"
    
    # Wait a moment and check if it's still running
    sleep 2
    
    if ps -p "$PID" > /dev/null 2>&1; then
        print_success "Backend server started (PID: $PID)"
        print_info "Server running at: http://localhost:8080"
        print_info "Logs: $LOG_FILE"
        print_info "Use '$0 stop' to stop the server"
        print_info "Use '$0 logs' to view logs"
        return 0
    else
        print_error "Failed to start backend server"
        print_info "Check logs: tail -f $LOG_FILE"
        rm -f "$PID_FILE"
        return 1
    fi
}

# Function to restart the server
restart_server() {
    print_info "Restarting backend server..."
    stop_server
    sleep 1
    start_server
}

# Function to show status
show_status() {
    if is_running; then
        PID=$(get_pid)
        print_success "Backend server is running (PID: $PID)"
        
        # Check if port 8080 is responding
        if curl -s http://localhost:8080/health > /dev/null 2>&1; then
            print_success "Server is responding at http://localhost:8080"
        else
            print_warning "Server process is running but not responding"
        fi
        
        # Show resource usage
        echo ""
        print_info "Resource usage:"
        ps -p "$PID" -o pid,ppid,%cpu,%mem,etime,command | tail -n +2
    else
        print_info "Backend server is not running"
        
        # Check if something else is on port 8080
        if lsof -i :8080 > /dev/null 2>&1; then
            print_warning "Port 8080 is in use by another process:"
            lsof -i :8080
        fi
    fi
}

# Function to show logs
show_logs() {
    if [ -f "$LOG_FILE" ]; then
        tail -f "$LOG_FILE"
    else
        print_error "Log file not found: $LOG_FILE"
    fi
}

# Function to show help
show_help() {
    cat << EOF
Email Client Backend Runner

Usage: $0 [COMMAND]

Commands:
    start       Start the backend server
    stop        Stop the backend server
    restart     Restart the backend server
    status      Show server status
    logs        Show and follow server logs
    build       Build the backend
    kill        Kill any process using port 8080
    clean       Stop server and clean up logs
    help        Show this help message

Examples:
    $0 start        # Start the server
    $0 stop         # Stop the server
    $0 restart      # Restart the server
    $0 status       # Check if server is running
    $0 logs         # View live logs
    $0 kill         # Kill process on port 8080

Server Details:
    URL:        http://localhost:8080
    PID File:   $PID_FILE
    Log File:   $LOG_FILE
    Backend:    $BACKEND_DIR

EOF
}

# Main script logic
case "${1:-start}" in
    start)
        start_server
        ;;
    stop)
        stop_server
        ;;
    restart)
        restart_server
        ;;
    status)
        show_status
        ;;
    logs)
        show_logs
        ;;
    build)
        build_backend
        ;;
    kill)
        kill_by_port 8080
        ;;
    clean)
        stop_server
        rm -f "$LOG_FILE"
        print_success "Cleaned up logs and stopped server"
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        print_error "Unknown command: $1"
        echo ""
        show_help
        exit 1
        ;;
esac
