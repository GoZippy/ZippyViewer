#!/bin/bash
# Health Check Script for coturn
# Used by load balancers and monitoring systems

set -e

HOST="${1:-127.0.0.1}"
PORT="${2:-3478}"
USERNAME="${3:-}"
PASSWORD="${4:-}"

# Simple STUN health check
if [ -z "$USERNAME" ] || [ -z "$PASSWORD" ]; then
    # Basic STUN check
    if command -v turnutils_stunclient &> /dev/null; then
        if turnutils_stunclient "$HOST" -p "$PORT" > /dev/null 2>&1; then
            echo "OK: STUN check passed"
            exit 0
        else
            echo "FAIL: STUN check failed"
            exit 1
        fi
    elif docker ps | grep -q zrc-coturn; then
        if docker exec zrc-coturn turnutils_stunclient "$HOST" -p "$PORT" > /dev/null 2>&1; then
            echo "OK: STUN check passed"
            exit 0
        else
            echo "FAIL: STUN check failed"
            exit 1
        fi
    else
        echo "FAIL: turnutils_stunclient not available"
        exit 1
    fi
else
    # Comprehensive TURN allocation check
    if command -v turnutils_peer &> /dev/null; then
        if turnutils_peer -L "$HOST" -p "$PORT" -U "$USERNAME" -P "$PASSWORD" > /dev/null 2>&1; then
            echo "OK: TURN allocation check passed"
            exit 0
        else
            echo "FAIL: TURN allocation check failed"
            exit 1
        fi
    elif docker ps | grep -q zrc-coturn; then
        if docker exec zrc-coturn turnutils_peer -L "$HOST" -p "$PORT" -U "$USERNAME" -P "$PASSWORD" > /dev/null 2>&1; then
            echo "OK: TURN allocation check passed"
            exit 0
        else
            echo "FAIL: TURN allocation check failed"
            exit 1
        fi
    else
        echo "FAIL: turnutils_peer not available"
        exit 1
    fi
fi
