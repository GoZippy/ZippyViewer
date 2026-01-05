#!/bin/bash
# Resource Limit Monitoring Script for coturn
# Tracks usage and alerts when approaching limits

LOG_FILE="${1:-./logs/turn.log}"
ALERT_THRESHOLD="${2:-80}"  # Alert when usage exceeds this percentage

if [ ! -f "$LOG_FILE" ]; then
    echo "Error: Log file not found: $LOG_FILE"
    exit 1
fi

echo "Monitoring coturn resource limits..."
echo "Alert threshold: ${ALERT_THRESHOLD}%"
echo ""

# Extract limits from configuration (if available)
MAX_BPS="${MAX_BPS:-10000000}"
MAX_ALLOCATIONS="${MAX_ALLOCATIONS:-1000}"
USER_QUOTA="${USER_QUOTA:-12}"

# Function to parse log for current usage
get_current_usage() {
    # This is a simplified version - actual implementation would parse coturn logs
    # or use coturn's management interface if available
    
    local active_allocations=$(tail -n 1000 "$LOG_FILE" | grep -c "session.*active" || echo "0")
    local total_bandwidth=$(tail -n 1000 "$LOG_FILE" | grep -oP "bps=\K[0-9]+" | awk '{sum+=$1} END {print sum}' || echo "0")
    
    echo "$active_allocations|$total_bandwidth"
}

# Function to check and alert
check_limits() {
    local usage=$(get_current_usage)
    local allocations=$(echo "$usage" | cut -d'|' -f1)
    local bandwidth=$(echo "$usage" | cut -d'|' -f2)
    
    # Check allocation limit
    local alloc_percent=$((allocations * 100 / MAX_ALLOCATIONS))
    if [ "$alloc_percent" -ge "$ALERT_THRESHOLD" ]; then
        echo "⚠ ALERT: Allocation usage at ${alloc_percent}% (${allocations}/${MAX_ALLOCATIONS})"
    else
        echo "✓ Allocations: ${allocations}/${MAX_ALLOCATIONS} (${alloc_percent}%)"
    fi
    
    # Check bandwidth limit (per user, simplified)
    local bps_per_user=$((bandwidth / allocations)) 2>/dev/null || echo "0"
    local bps_percent=$((bps_per_user * 100 / MAX_BPS))
    if [ "$bps_percent" -ge "$ALERT_THRESHOLD" ]; then
        echo "⚠ ALERT: Bandwidth usage at ${bps_percent}% (${bps_per_user} bps / ${MAX_BPS} bps per user)"
    else
        echo "✓ Bandwidth: ${bps_per_user} bps / ${MAX_BPS} bps per user (${bps_percent}%)"
    fi
}

# Run check
check_limits

echo ""
echo "Note: This is a basic monitoring script. For production, consider:"
echo "  - Integrating with Prometheus/Grafana"
echo "  - Using coturn's management interface (if available)"
echo "  - Setting up automated alerts"
