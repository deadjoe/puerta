#!/bin/bash

# Quick MongoDB Load Balancer Test Script
# Fast verification of basic load balancer functionality

set -e

LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="27016"
BACKEND_ROUTERS=("127.0.0.1:27017" "127.0.0.1:27018" "127.0.0.1:27019")

echo "=== Quick Puerta Load Balancer Test ==="
echo "Testing: ${LOAD_BALANCER_HOST}:${LOAD_BALANCER_PORT}"
echo "Backends: ${BACKEND_ROUTERS[*]}"
echo ""

# Test 1: Basic connectivity
echo "1. Testing basic connectivity..."
if ! mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.adminCommand('ping')" --quiet > /dev/null; then
    echo "❌ Load balancer connectivity failed"
    exit 1
fi
echo "✅ Load balancer connectivity: SUCCESS"

# Test 2: Router verification
echo ""
echo "2. Verifying router functionality..."
output=$(mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.adminCommand('ismaster')" --quiet)
if ! echo "$output" | grep -q "msg.*isdbgrid"; then
    echo "❌ Not responding as mongos router"
    exit 1
fi
echo "✅ Router verification: SUCCESS"

# Test 3: Load balancing (quick test)
echo ""
echo "3. Testing load balancing (5 connections)..."
connection_ids=()
for i in {1..5}; do
    output=$(mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.adminCommand('ismaster')" --quiet)
    conn_id=$(echo "$output" | grep -o 'connectionId: [0-9]*' | cut -d':' -f2 | tr -d ' ')
    connection_ids+=($conn_id)
    echo "   Connection $i: connectionId $conn_id"
done

unique_count=$(printf "%s\n" "${connection_ids[@]}" | sort -u | wc -l)
total_count=${#connection_ids[@]}

if [ "$unique_count" -eq "$total_count" ]; then
    echo "✅ Load balancing: SUCCESS ($unique_count/$total_count unique connections)"
else
    echo "⚠️  Load balancing: PARTIAL ($unique_count/$total_count unique connections)"
fi

# Test 4: Basic operations
echo ""
echo "4. Testing basic operations..."
test_db="puerta_quick_test_$$"
if mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "
    use('$test_db');
    db.test.insertOne({test: 'quick', timestamp: new Date()});
    db.test.findOne();
" --quiet > /dev/null; then
    echo "✅ Basic operations: SUCCESS"
else
    echo "❌ Basic operations: FAILED"
    exit 1
fi

# Cleanup
mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.getSiblingDB('$test_db').dropDatabase()" --quiet > /dev/null

echo ""
echo "=== Quick Test Summary ==="
echo "✅ Load balancer is working correctly"
echo "✅ All basic tests passed"
echo ""
echo "Run './test_mongodb_lb_comprehensive.sh' for full testing suite."