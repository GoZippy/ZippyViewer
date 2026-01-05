#!/bin/bash
# Log Monitoring Script for coturn
# Monitors logs for events, errors, and potential issues

LOG_FILE="${1:-./logs/turn.log}"
ALERT_ON_ERRORS="${2:-true}"

if [ ! -f "$LOG_FILE" ]; then
    echo "Error: Log file not found: $LOG_FILE"
    echo "Make sure coturn is running and logging to this file"
    exit 1
fi

echo "Monitoring coturn logs: $LOG_FILE"
echo "Press Ctrl+C to stop"
echo ""

# Function to check for errors
check_errors() {
    local errors=$(tail -n 100 "$LOG_FILE" | grep -i "error\|fail\|denied" | wc -l)
    if [ "$errors" -gt 0 ] && [ "$ALERT_ON_ERRORS" = "true" ]; then
        echo "⚠ ALERT: Found $errors error(s) in recent logs"
        tail -n 100 "$LOG_FILE" | grep -i "error\|fail\|denied" | tail -5
    fi
}

# Function to show statistics
show_stats() {
    local allocations=$(tail -n 1000 "$LOG_FILE" | grep -c "session" || echo "0")
    local auth_failures=$(tail -n 1000 "$LOG_FILE" | grep -ci "auth.*fail\|unauthorized" || echo "0")
    local bandwidth=$(tail -n 1000 "$LOG_FILE" | grep -oP "bps=\K[0-9]+" | awk '{sum+=$1} END {print sum/1000000 " Mbps"}' || echo "0 Mbps")
    
    echo "Recent Statistics (last 1000 log entries):"
    echo "  Allocations: $allocations"
    echo "  Auth Failures: $auth_failures"
    echo "  Bandwidth: $bandwidth"
}

# Tail the log file
tail -f "$LOG_FILE" | while read line; do
    echo "$line"
    
    # Check for critical errors
    if echo "$line" | grep -qi "fatal\|critical\|cannot bind"; then
        echo "🚨 CRITICAL: $line"
    fi
    
    # Check for authentication failures
    if echo "$line" | grep -qi "auth.*fail\|unauthorized"; then
        echo "⚠ AUTH FAILURE: $line"
    fi
    
    # Check for allocation events
    if echo "$line" | grep -qi "allocation\|session.*created"; then
        echo "ℹ ALLOCATION: $line"
    fi
done
