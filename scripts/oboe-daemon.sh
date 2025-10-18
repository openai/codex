#!/bin/bash
#
# UNOVA Oboe Daemon Startup Script
#
# This script provides an easy way to start, stop, and check the status of the UNOVA Oboe daemon.
#

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DAEMON_SCRIPT="$PROJECT_ROOT/scripts/oboe_daemon.py"
PID_FILE="$HOME/.codex/oboe-daemon.pid"
CONFIG_FILE="$HOME/.codex/oboe-config.json"

# Ensure .codex directory exists
mkdir -p "$HOME/.codex/log"

# Function to show usage
usage() {
    echo "Usage: $0 {start|stop|restart|status|help}"
    echo
    echo "Commands:"
    echo "  start    - Start the UNOVA Oboe daemon"
    echo "  stop     - Stop the UNOVA Oboe daemon"
    echo "  restart  - Restart the UNOVA Oboe daemon"
    echo "  status   - Show daemon and service status"
    echo "  help     - Show this help message"
    echo
    echo "Environment variables:"
    echo "  OBOE_CONFIG - Path to configuration file (default: $CONFIG_FILE)"
}

# Function to start the daemon
start_daemon() {
    if [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
        echo "UNOVA Oboe daemon is already running (PID: $(cat "$PID_FILE"))"
        return 1
    fi

    echo "Starting UNOVA Oboe daemon..."
    
    # Use custom config if specified
    CONFIG_ARG=""
    if [ -n "$OBOE_CONFIG" ]; then
        CONFIG_ARG="--config $OBOE_CONFIG"
    elif [ -f "$CONFIG_FILE" ]; then
        CONFIG_ARG="--config $CONFIG_FILE"
    fi

    # Start the daemon in background
    python3 "$DAEMON_SCRIPT" $CONFIG_ARG --daemon &
    DAEMON_PID=$!
    
    # Save PID
    echo $DAEMON_PID > "$PID_FILE"
    
    # Wait a moment to see if it started successfully
    sleep 2
    if kill -0 $DAEMON_PID 2>/dev/null; then
        echo "UNOVA Oboe daemon started successfully (PID: $DAEMON_PID)"
        return 0
    else
        echo "Failed to start UNOVA Oboe daemon"
        rm -f "$PID_FILE"
        return 1
    fi
}

# Function to stop the daemon
stop_daemon() {
    if [ ! -f "$PID_FILE" ]; then
        echo "UNOVA Oboe daemon is not running"
        return 1
    fi

    PID=$(cat "$PID_FILE")
    if ! kill -0 "$PID" 2>/dev/null; then
        echo "UNOVA Oboe daemon is not running (stale PID file)"
        rm -f "$PID_FILE"
        return 1
    fi

    echo "Stopping UNOVA Oboe daemon (PID: $PID)..."
    kill "$PID"
    
    # Wait for it to stop
    for i in {1..10}; do
        if ! kill -0 "$PID" 2>/dev/null; then
            rm -f "$PID_FILE"
            echo "UNOVA Oboe daemon stopped"
            return 0
        fi
        sleep 1
    done

    # Force kill if it didn't stop gracefully
    echo "Force stopping UNOVA Oboe daemon..."
    kill -9 "$PID" 2>/dev/null
    rm -f "$PID_FILE"
    echo "UNOVA Oboe daemon stopped (forced)"
    return 0
}

# Function to show status
show_status() {
    echo "=== UNOVA Oboe Daemon Status ==="
    
    # Check if daemon is running
    if [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
        echo "Daemon Status: RUNNING (PID: $(cat "$PID_FILE"))"
    else
        echo "Daemon Status: STOPPED"
        return 1
    fi
    
    echo
    echo "=== Service Status ==="
    
    # Get service status from daemon
    CONFIG_ARG=""
    if [ -n "$OBOE_CONFIG" ]; then
        CONFIG_ARG="--config $OBOE_CONFIG"
    elif [ -f "$CONFIG_FILE" ]; then
        CONFIG_ARG="--config $CONFIG_FILE"
    fi
    
    python3 "$DAEMON_SCRIPT" $CONFIG_ARG --status
}

# Function to restart the daemon
restart_daemon() {
    echo "Restarting UNOVA Oboe daemon..."
    stop_daemon
    sleep 1
    start_daemon
}

# Main script logic
case "$1" in
    start)
        start_daemon
        ;;
    stop)
        stop_daemon
        ;;
    restart)
        restart_daemon
        ;;
    status)
        show_status
        ;;
    help|--help|-h)
        usage
        ;;
    *)
        echo "Invalid command: $1"
        echo
        usage
        exit 1
        ;;
esac

exit $?