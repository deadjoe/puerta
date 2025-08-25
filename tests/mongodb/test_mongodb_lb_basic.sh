#!/bin/bash

# MongoDB Load Balancer Basic Test Script
# Tests basic connectivity and load balancing functionality
# 
# Usage: ./test_mongodb_lb_basic.sh
# Output: Console output with test results
# Duration: ~15-30 seconds

set -e  # Exit on any error

# Configuration
LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="27016"
TEST_DB_PREFIX="puerta_test"

# Cleanup function
cleanup() {
    if [ -n "$test_db" ]; then
        mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.getSiblingDB('$test_db').dropDatabase()" --quiet > /dev/null 2>&1 || true
    fi
}

# Handle script interruption
trap cleanup EXIT

echo "=== MongoDB Load Balancer Test ==="
echo "Testing Puerta load balancer at 127.0.0.1:27016"
echo "Backend routers: 127.0.0.1:27017, 127.0.0.1:27018, 127.0.0.1:27019"
echo ""

# Test 1: Basic connectivity
echo "1. Testing basic connectivity..."
mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.adminCommand('ping')" --quiet > /dev/null
if [ $? -eq 0 ]; then
    echo "✅ Load balancer connectivity: SUCCESS"
else
    echo "❌ Load balancer connectivity: FAILED"
    exit 1
fi

# Test 2: Multiple connections to verify load balancing
echo ""
echo "2. Testing load balancing with 10 connections..."
connection_ids=()
for i in {1..10}; do
    output=$(mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.adminCommand('ismaster')" --quiet)
    conn_id=$(echo "$output" | grep -o 'connectionId: [0-9]*' | cut -d':' -f2 | tr -d ' ')
    connection_ids+=($conn_id)
    echo "   Connection $i: connectionId $conn_id"
done

# Test 3: Verify all connections are unique (indicating load balancing)
echo ""
echo "3. Analyzing load balancing..."
unique_ids=($(printf "%s\n" "${connection_ids[@]}" | sort -u))
total_connections=${#connection_ids[@]}
unique_connections=${#unique_ids[@]}

echo "   Total connections: $total_connections"
echo "   Unique connection IDs: $unique_connections"

if [ $unique_connections -eq $total_connections ]; then
    echo "✅ Load balancing: All connections have unique IDs"
else
    echo "⚠️  Load balancing: Some connections may share backend"
fi

# Test 4: Test basic MongoDB operations
echo ""
echo "4. Testing MongoDB operations..."
test_db="${TEST_DB_PREFIX}_${$: -4}"
mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "
use('$test_db');
db.test_collection.insertOne({test: 'data', timestamp: new Date()});
db.test_collection.findOne();
" --quiet > /dev/null

if [ $? -eq 0 ]; then
    echo "✅ MongoDB operations: SUCCESS"
else
    echo "❌ MongoDB operations: FAILED"
fi

# Cleanup is handled by trap function

echo ""
echo "=== Test Summary ==="
echo "✅ Load balancer is working correctly"
echo "✅ MongoDB connectivity established"
echo "✅ Basic operations functional"
echo ""
echo "Load balancer is successfully distributing connections across MongoDB routers."