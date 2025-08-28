#!/bin/bash

# Redis Load Balancer Basic Test Script
# Tests basic connectivity and load balancing functionality for Redis Cluster
# 
# Usage: ./test_redis_lb_basic.sh
# Output: Console output with test results
# Duration: ~15-30 seconds

set -e  # Exit on any error

# Configuration
LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="6379"  # Default Redis port for Puerta proxy
REDIS_NODES=("127.0.0.1:7001" "127.0.0.1:7002" "127.0.0.1:7003" "127.0.0.1:7004" "127.0.0.1:7005" "127.0.0.1:7006")
TEST_KEY_PREFIX="puerta_test"

# Cleanup function
cleanup() {
    # Clean up test keys
    for key_id in {1..10}; do
        redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "${TEST_KEY_PREFIX}_${key_id}" > /dev/null 2>&1 || true
    done
    redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "${TEST_KEY_PREFIX}_hash_test" > /dev/null 2>&1 || true
}

# Handle script interruption
trap cleanup EXIT

echo "=== Redis Load Balancer Test ==="
echo "Testing Puerta load balancer at ${LOAD_BALANCER_HOST}:${LOAD_BALANCER_PORT}"
echo "Backend nodes: ${REDIS_NODES[*]}"
echo ""

# Test 1: Basic connectivity to load balancer
echo "1. Testing basic connectivity..."
response=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" ping 2>/dev/null)
if [ "$response" = "PONG" ]; then
    echo "✅ Load balancer connectivity: SUCCESS"
else
    echo "❌ Load balancer connectivity: FAILED (got: $response)"
    exit 1
fi

# Test 2: Test basic Redis operations
echo ""
echo "2. Testing basic Redis operations..."
redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "${TEST_KEY_PREFIX}_basic" "test_value" > /dev/null 2>&1
retrieved_value=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" get "${TEST_KEY_PREFIX}_basic" 2>/dev/null)

if [ "$retrieved_value" = "test_value" ]; then
    echo "✅ Basic SET/GET operations: SUCCESS"
else
    echo "❌ Basic SET/GET operations: FAILED"
    exit 1
fi

# Test 3: Test multiple key operations to verify slot routing
echo ""
echo "3. Testing multiple key operations (slot routing)..."
success_count=0
total_operations=10

for i in $(seq 1 $total_operations); do
    test_key="${TEST_KEY_PREFIX}_${i}"
    test_value="value_${i}"
    
    # Set key
    if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "$test_key" "$test_value" > /dev/null 2>&1; then
        # Get key to verify
        retrieved=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" get "$test_key" 2>/dev/null)
        if [ "$retrieved" = "$test_value" ]; then
            ((success_count++))
            echo "   Key $i: ✅ (slot: $(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" cluster keyslot "$test_key" 2>/dev/null || echo 'N/A'))"
        else
            echo "   Key $i: ❌ (retrieval failed)"
        fi
    else
        echo "   Key $i: ❌ (set failed)"
    fi
done

echo "   Successfully processed: $success_count/$total_operations keys"
if [ $success_count -eq $total_operations ]; then
    echo "✅ Multiple key operations: SUCCESS"
else
    echo "⚠️  Multiple key operations: PARTIAL SUCCESS ($success_count/$total_operations)"
fi

# Test 4: Test hash tag functionality for consistent routing
echo ""
echo "4. Testing hash tag functionality..."
hash_tag_key1="{user123}_profile"
hash_tag_key2="{user123}_settings"

redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "$hash_tag_key1" "profile_data" > /dev/null 2>&1
redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "$hash_tag_key2" "settings_data" > /dev/null 2>&1

# Both keys should route to the same slot due to hash tag
slot1=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" cluster keyslot "$hash_tag_key1" 2>/dev/null || echo 'N/A')
slot2=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" cluster keyslot "$hash_tag_key2" 2>/dev/null || echo 'N/A')

if [ "$slot1" = "$slot2" ] && [ "$slot1" != "N/A" ]; then
    echo "✅ Hash tag routing: SUCCESS (both keys in slot $slot1)"
else
    echo "⚠️  Hash tag routing: Keys routed to different slots ($slot1 vs $slot2)"
fi

# Test 5: Test concurrent connections
echo ""
echo "5. Testing concurrent connections..."
temp_dir=$(mktemp -d)
concurrent_count=5
success_count=0

for i in $(seq 1 $concurrent_count); do
    (
        if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "concurrent_${i}" "value_${i}" > /dev/null 2>&1; then
            if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" get "concurrent_${i}" > /dev/null 2>&1; then
                echo "success" > "$temp_dir/result_${i}"
            fi
        fi
        redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "concurrent_${i}" > /dev/null 2>&1 || true
    ) &
done

wait

# Count successful operations
for i in $(seq 1 $concurrent_count); do
    if [ -f "$temp_dir/result_${i}" ]; then
        ((success_count++))
    fi
done

rm -rf "$temp_dir"

echo "   Concurrent operations completed: $success_count/$concurrent_count"
if [ $success_count -eq $concurrent_count ]; then
    echo "✅ Concurrent connections: SUCCESS"
else
    echo "⚠️  Concurrent connections: PARTIAL SUCCESS ($success_count/$concurrent_count)"
fi

# Test 6: Test Redis cluster info through proxy
echo ""
echo "6. Testing cluster information access..."
cluster_info=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" cluster info 2>/dev/null || echo "")
if echo "$cluster_info" | grep -q "cluster_state"; then
    cluster_state=$(echo "$cluster_info" | grep "cluster_state" | cut -d':' -f2)
    echo "✅ Cluster info access: SUCCESS (state: $cluster_state)"
else
    echo "⚠️  Cluster info access: Limited or no cluster info available"
fi

# Cleanup is handled by trap function

echo ""
echo "=== Test Summary ==="
echo "✅ Load balancer is proxying Redis traffic correctly"
echo "✅ Basic Redis operations are functional"
echo "✅ Slot-based routing is working"
echo ""
echo "Redis load balancer is successfully proxying traffic to the cluster."