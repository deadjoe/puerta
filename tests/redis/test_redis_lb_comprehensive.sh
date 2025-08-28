#!/bin/bash

# Enhanced Redis Load Balancer Test Suite
# Comprehensive testing for Puerta Redis load balancer
# Tests connectivity, load balancing, performance, fault tolerance, and Redis-specific features

# Don't exit on error - we want to run all tests and report results

# Configuration
LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="6379"
REDIS_NODES=("127.0.0.1:7001" "127.0.0.1:7002" "127.0.0.1:7003" "127.0.0.1:7004" "127.0.0.1:7005" "127.0.0.1:7006")
TEST_DB_PREFIX="puerta_redis_test"
LOG_FILE="/tmp/puerta_redis_test_$(date +%Y%m%d_%H%M%S).log"
REPORT_FILE="/tmp/puerta_redis_test_report_$(date +%Y%m%d_%H%M%S).json"

# Test configuration
CONCURRENT_CONNECTIONS=20
STRESS_TEST_DURATION=30
BATCH_SIZE=100
MAX_RETRIES=3

# Global variables
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
START_TIME=$(date +%s)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Utility functions
log() {
    echo -e "$1" | tee -a "$LOG_FILE"
}

log_info() {
    log "${BLUE}[INFO]${NC} $1"
}

log_success() {
    log "${GREEN}[SUCCESS]${NC} $1"
    ((PASSED_TESTS++))
}

log_error() {
    log "${RED}[ERROR]${NC} $1"
    ((FAILED_TESTS++))
}

log_warning() {
    log "${YELLOW}[WARNING]${NC} $1"
}

start_test() {
    ((TOTAL_TESTS++))
    log_info "Starting test: $1"
}

end_test() {
    local result=$1
    if [ $result -eq 0 ]; then
        log_success "Test passed: $2"
    else
        log_error "Test failed: $2"
    fi
}

# Initialize test environment
init_test() {
    log_info "=== Puerta Redis Load Balancer Comprehensive Test Suite ==="
    log_info "Load Balancer: ${LOAD_BALANCER_HOST}:${LOAD_BALANCER_PORT}"
    log_info "Redis Nodes: ${REDIS_NODES[*]}"
    log_info "Log File: ${LOG_FILE}"
    log_info "Report File: ${REPORT_FILE}"
    log_info "Start Time: $(date)"
    echo "" | tee -a "$LOG_FILE"
    
    # Clean up any existing test keys
    cleanup_test_keys
    
    # Initialize report JSON
    cat > "$REPORT_FILE" << EOF
{
  "test_info": {
    "load_balancer": "${LOAD_BALANCER_HOST}:${LOAD_BALANCER_PORT}",
    "redis_nodes": [$(printf '"%s",' "${REDIS_NODES[@]}" | sed 's/,$//')],
    "start_time": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "duration_seconds": 0
  },
  "test_results": [],
  "performance_metrics": {},
  "redis_specific_tests": {},
  "error_analysis": {}
}
EOF
}

# Clean up test keys
cleanup_test_keys() {
    log_info "Cleaning up test keys..."
    for i in {1..100}; do
        redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "${TEST_DB_PREFIX}_${i}" > /dev/null 2>&1 || true
        redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "stress_${i}" > /dev/null 2>&1 || true
        redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "concurrent_${i}" > /dev/null 2>&1 || true
    done
    redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "hash_test_key" "{user123}_*" > /dev/null 2>&1 || true
}

# Test 1: Basic connectivity test
test_basic_connectivity() {
    start_test "Basic Connectivity Test"
    
    local success=true
    
    # Test load balancer connectivity
    if ! redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" ping > /dev/null 2>&1; then
        log_error "Load balancer connectivity failed"
        success=false
    else
        log_info "Load balancer connectivity: OK"
    fi
    
    # Test direct node connectivity
    local available_nodes=0
    for node in "${REDIS_NODES[@]}"; do
        local host=${node%:*}
        local port=${node#*:}
        if redis-cli -h "$host" -p "$port" ping > /dev/null 2>&1; then
            ((available_nodes++))
        fi
    done
    
    log_info "Available Redis nodes: $available_nodes/${#REDIS_NODES[@]}"
    
    if [ $available_nodes -eq 0 ]; then
        log_error "No Redis nodes available"
        success=false
    fi
    
    if $success; then
        end_test 0 "Basic Connectivity"
    else
        end_test 1 "Basic Connectivity"
    fi
}

# Test 2: Redis cluster information
test_cluster_info() {
    start_test "Redis Cluster Information Test"
    
    local success=true
    
    # Get cluster info through proxy
    local cluster_info=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" cluster info 2>/dev/null)
    if [ -n "$cluster_info" ]; then
        local cluster_state=$(echo "$cluster_info" | grep "cluster_state" | cut -d':' -f2 | tr -d '\r')
        local cluster_size=$(echo "$cluster_info" | grep "cluster_size" | cut -d':' -f2 | tr -d '\r')
        local slots_assigned=$(echo "$cluster_info" | grep "cluster_slots_assigned" | cut -d':' -f2 | tr -d '\r')
        
        log_info "Cluster state: $cluster_state"
        log_info "Cluster size: $cluster_size"
        log_info "Slots assigned: $slots_assigned"
        
        if [ "$cluster_state" = "ok" ]; then
            log_success "Cluster is in OK state"
        else
            log_warning "Cluster state is: $cluster_state"
        fi
    else
        log_error "Failed to get cluster information"
        success=false
    fi
    
    # Get cluster nodes
    local cluster_nodes=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" cluster nodes 2>/dev/null)
    if [ -n "$cluster_nodes" ]; then
        local node_count=$(echo "$cluster_nodes" | wc -l | tr -d ' ')
        log_info "Cluster nodes visible: $node_count"
    fi
    
    if $success; then
        end_test 0 "Cluster Information"
    else
        end_test 1 "Cluster Information"
    fi
}

# Test 3: Basic Redis operations
test_basic_operations() {
    start_test "Basic Redis Operations Test"
    
    local success=true
    
    # Test basic string operations
    if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "test_string" "hello_redis" > /dev/null 2>&1; then
        local value=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" get "test_string" 2>/dev/null)
        if [ "$value" = "hello_redis" ]; then
            log_info "String operations: OK"
        else
            log_error "String GET operation failed"
            success=false
        fi
    else
        log_error "String SET operation failed"
        success=false
    fi
    
    # Test hash operations
    if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" hset "test_hash" "field1" "value1" > /dev/null 2>&1; then
        local hash_value=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" hget "test_hash" "field1" 2>/dev/null)
        if [ "$hash_value" = "value1" ]; then
            log_info "Hash operations: OK"
        else
            log_error "Hash HGET operation failed"
            success=false
        fi
    else
        log_error "Hash HSET operation failed"
        success=false
    fi
    
    # Test list operations
    if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" lpush "test_list" "item1" "item2" > /dev/null 2>&1; then
        local list_length=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" llen "test_list" 2>/dev/null)
        if [ "$list_length" = "2" ]; then
            log_info "List operations: OK"
        else
            log_error "List LLEN operation failed"
            success=false
        fi
    else
        log_error "List LPUSH operation failed"
        success=false
    fi
    
    # Cleanup
    redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "test_string" "test_hash" "test_list" > /dev/null 2>&1 || true
    
    if $success; then
        end_test 0 "Basic Operations"
    else
        end_test 1 "Basic Operations"
    fi
}

# Test 4: Slot routing and hash tags
test_slot_routing() {
    start_test "Slot Routing and Hash Tags Test"
    
    local success=true
    
    # Test slot calculation
    local key1="user123_profile"
    local key2="user456_profile"
    
    local slot1=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" cluster keyslot "$key1" 2>/dev/null)
    local slot2=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" cluster keyslot "$key2" 2>/dev/null)
    
    if [ -n "$slot1" ] && [ -n "$slot2" ] && [ "$slot1" != "$slot2" ]; then
        log_info "Different keys route to different slots: $key1->$slot1, $key2->$slot2"
    else
        log_warning "Slot calculation may not be working correctly"
    fi
    
    # Test hash tags for consistent routing
    local hash_key1="{user789}_profile"
    local hash_key2="{user789}_settings"
    
    local hash_slot1=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" cluster keyslot "$hash_key1" 2>/dev/null)
    local hash_slot2=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" cluster keyslot "$hash_key2" 2>/dev/null)
    
    if [ -n "$hash_slot1" ] && [ "$hash_slot1" = "$hash_slot2" ]; then
        log_success "Hash tags route to same slot: both keys -> slot $hash_slot1"
        
        # Test actual operations with hash tags
        redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "$hash_key1" "profile_data" > /dev/null 2>&1
        redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "$hash_key2" "settings_data" > /dev/null 2>&1
        
        local retrieved1=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" get "$hash_key1" 2>/dev/null)
        local retrieved2=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" get "$hash_key2" 2>/dev/null)
        
        if [ "$retrieved1" = "profile_data" ] && [ "$retrieved2" = "settings_data" ]; then
            log_success "Hash tag operations successful"
        else
            log_error "Hash tag operations failed"
            success=false
        fi
        
        # Cleanup
        redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "$hash_key1" "$hash_key2" > /dev/null 2>&1 || true
    else
        log_error "Hash tags not routing to same slot: $hash_key1->$hash_slot1, $hash_key2->$hash_slot2"
        success=false
    fi
    
    if $success; then
        end_test 0 "Slot Routing"
    else
        end_test 1 "Slot Routing"
    fi
}

# Test 5: Concurrent connections
test_concurrent_connections() {
    start_test "Concurrent Connections Test"
    
    local success=true
    local temp_dir=$(mktemp -d)
    local connection_count=0
    
    log_info "Testing $CONCURRENT_CONNECTIONS concurrent connections..."
    
    # Create concurrent connection scripts
    for i in $(seq 1 $CONCURRENT_CONNECTIONS); do
        (
            if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "concurrent_$i" "value_$i" > /dev/null 2>&1; then
                if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" get "concurrent_$i" > /dev/null 2>&1; then
                    echo "success" > "$temp_dir/conn_$i.result"
                fi
            fi
            redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "concurrent_$i" > /dev/null 2>&1 || true
        ) &
    done
    
    # Wait for all connections to complete
    wait
    
    # Count successful connections
    for i in $(seq 1 $CONCURRENT_CONNECTIONS); do
        if [ -f "$temp_dir/conn_$i.result" ]; then
            ((connection_count++))
        fi
    done
    
    log_info "Concurrent connections successful: $connection_count/$CONCURRENT_CONNECTIONS"
    
    # Check results
    if [ "$connection_count" -eq "$CONCURRENT_CONNECTIONS" ]; then
        log_success "All concurrent connections successful"
    elif [ "$connection_count" -gt $((CONCURRENT_CONNECTIONS * 8 / 10)) ]; then
        log_warning "Most concurrent connections successful"
    else
        log_error "Many concurrent connections failed"
        success=false
    fi
    
    # Cleanup
    rm -rf "$temp_dir"
    
    if $success; then
        end_test 0 "Concurrent Connections"
    else
        end_test 1 "Concurrent Connections"
    fi
}

# Test 6: Performance benchmark
test_performance_benchmark() {
    start_test "Performance Benchmark Test"
    
    local success=true
    local test_start=$(date +%s)
    
    log_info "Running performance benchmark..."
    
    # Test SET performance
    local set_start=$(date +%s)
    local set_operations=100
    local set_success=0
    
    for i in $(seq 1 $set_operations); do
        if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "perf_test_$i" "value_$i" > /dev/null 2>&1; then
            ((set_success++))
        fi
    done
    
    local set_end=$(date +%s)
    local set_duration=$((set_end - set_start))
    
    # Test GET performance
    local get_start=$(date +%s)
    local get_operations=100
    local get_success=0
    
    for i in $(seq 1 $get_operations); do
        if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" get "perf_test_$i" > /dev/null 2>&1; then
            ((get_success++))
        fi
    done
    
    local get_end=$(date +%s)
    local get_duration=$((get_end - get_start))
    
    # Calculate performance metrics
    local set_ops_per_sec=0
    local get_ops_per_sec=0
    
    if [ $set_duration -gt 0 ]; then
        set_ops_per_sec=$((set_success / set_duration))
    else
        set_ops_per_sec=$set_success
    fi
    
    if [ $get_duration -gt 0 ]; then
        get_ops_per_sec=$((get_success / get_duration))
    else
        get_ops_per_sec=$get_success
    fi
    
    log_info "Performance Results:"
    log_info "  SET operations: $set_success/$set_operations in ${set_duration}s (${set_ops_per_sec} ops/sec)"
    log_info "  GET operations: $get_success/$get_operations in ${get_duration}s (${get_ops_per_sec} ops/sec)"
    
    # Cleanup performance test keys
    for i in $(seq 1 $set_operations); do
        redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "perf_test_$i" > /dev/null 2>&1 || true
    done
    
    # Save performance metrics
    cat >> "$REPORT_FILE" << EOF
  "performance_metrics": {
    "set_operations": {
      "count": $set_operations,
      "successful": $set_success,
      "duration_seconds": $set_duration,
      "ops_per_second": $set_ops_per_sec
    },
    "get_operations": {
      "count": $get_operations,
      "successful": $get_success,
      "duration_seconds": $get_duration,
      "ops_per_second": $get_ops_per_sec
    }
  },
EOF
    
    local test_end=$(date +%s)
    log_info "Performance benchmark completed in $((test_end - test_start))s"
    
    if $success; then
        end_test 0 "Performance Benchmark"
    else
        end_test 1 "Performance Benchmark"
    fi
}

# Test 7: Redis-specific data types
test_redis_data_types() {
    start_test "Redis Data Types Test"
    
    local success=true
    
    log_info "Testing Redis-specific data types..."
    
    # Test Sets
    if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" sadd "test_set" "member1" "member2" "member3" > /dev/null 2>&1; then
        local set_card=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" scard "test_set" 2>/dev/null)
        if [ "$set_card" = "3" ]; then
            log_info "Set operations: OK"
        else
            log_error "Set cardinality incorrect: expected 3, got $set_card"
            success=false
        fi
    else
        log_error "Set SADD operation failed"
        success=false
    fi
    
    # Test Sorted Sets
    if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" zadd "test_zset" 1 "first" 2 "second" 3 "third" > /dev/null 2>&1; then
        local zset_card=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" zcard "test_zset" 2>/dev/null)
        if [ "$zset_card" = "3" ]; then
            log_info "Sorted set operations: OK"
        else
            log_error "Sorted set cardinality incorrect: expected 3, got $zset_card"
            success=false
        fi
    else
        log_error "Sorted set ZADD operation failed"
        success=false
    fi
    
    # Test expiration
    if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" setex "test_expire" 2 "temporary" > /dev/null 2>&1; then
        sleep 1
        local ttl=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" ttl "test_expire" 2>/dev/null)
        if [ "$ttl" = "1" ] || [ "$ttl" = "0" ]; then
            log_info "Expiration operations: OK (TTL: $ttl)"
        else
            log_warning "Expiration TTL unexpected: $ttl"
        fi
    else
        log_error "SETEX operation failed"
        success=false
    fi
    
    # Cleanup
    redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "test_set" "test_zset" "test_expire" > /dev/null 2>&1 || true
    
    if $success; then
        end_test 0 "Redis Data Types"
    else
        end_test 1 "Redis Data Types"
    fi
}

# Test 8: Error handling
test_error_handling() {
    start_test "Error Handling Test"
    
    local success=true
    
    log_info "Testing error handling scenarios..."
    
    # Test invalid commands
    local invalid_output=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" invalid_command 2>&1 || true)
    if echo "$invalid_output" | grep -q "ERR unknown command"; then
        log_info "Invalid command handling: OK"
    else
        log_warning "Invalid command error handling unclear"
    fi
    
    # Test wrong number of arguments
    local wrong_args_output=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "key" 2>&1 || true)
    if echo "$wrong_args_output" | grep -q "wrong number of arguments"; then
        log_info "Wrong arguments handling: OK"
    else
        log_warning "Wrong arguments error handling unclear"
    fi
    
    # Test operation on wrong data type
    redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "string_key" "value" > /dev/null 2>&1
    local wrong_type_output=$(redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" lpush "string_key" "item" 2>&1 || true)
    if echo "$wrong_type_output" | grep -q "WRONGTYPE"; then
        log_info "Wrong type handling: OK"
    else
        log_warning "Wrong type error handling unclear"
    fi
    
    # Cleanup
    redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "string_key" > /dev/null 2>&1 || true
    
    log_success "Error handling tests completed"
    
    if $success; then
        end_test 0 "Error Handling"
    else
        end_test 1 "Error Handling"
    fi
}

# Test 9: Connection resilience
test_connection_resilience() {
    start_test "Connection Resilience Test"
    
    local success=true
    local failed_connections=0
    
    log_info "Testing connection resilience..."
    
    # Test multiple connection attempts
    for i in {1..20}; do
        local retry_count=0
        local connected=false
        
        while [ $retry_count -lt $MAX_RETRIES ]; do
            if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" ping > /dev/null 2>&1; then
                connected=true
                break
            fi
            ((retry_count++))
            sleep 1
        done
        
        if ! $connected; then
            ((failed_connections++))
            log_error "Connection $i failed after $MAX_RETRIES retries"
        fi
    done
    
    if [ $failed_connections -eq 0 ]; then
        log_success "All connection resilience tests passed"
    else
        log_error "$failed_connections connections failed resilience test"
        success=false
    fi
    
    if $success; then
        end_test 0 "Connection Resilience"
    else
        end_test 1 "Connection Resilience"
    fi
}

# Test 10: Stress test
test_stress_test() {
    start_test "Stress Test"
    
    local success=true
    local test_start=$(date +%s)
    local temp_dir=$(mktemp -d)
    
    log_info "Running stress test for $STRESS_TEST_DURATION seconds..."
    
    # Create stress test script
    cat > "$temp_dir/stress.sh" << 'EOF'
#!/bin/bash
LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="6379"
DURATION=$1
WORKER_ID=$2

end_time=$(($(date +%s) + DURATION))
operations=0

while [ $(date +%s) -lt $end_time ]; do
    key="stress_${WORKER_ID}_${operations}"
    value="data_${operations}"
    
    if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" set "$key" "$value" > /dev/null 2>&1; then
        if redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" get "$key" > /dev/null 2>&1; then
            ((operations++))
        fi
    fi
done

echo $operations
EOF
    
    chmod +x "$temp_dir/stress.sh"
    
    # Run concurrent stress tests
    local pids=()
    local ops_files=()
    
    for i in {1..3}; do
        local ops_file="$temp_dir/ops_$i.txt"
        ops_files+=("$ops_file")
        "$temp_dir/stress.sh" "$STRESS_TEST_DURATION" "$i" > "$ops_file" &
        pids+=($!)
    done
    
    # Wait for all stress tests to complete
    for pid in "${pids[@]}"; do
        wait "$pid"
    done
    
    # Calculate total operations
    local total_operations=0
    for ops_file in "${ops_files[@]}"; do
        local ops=$(cat "$ops_file" 2>/dev/null || echo 0)
        total_operations=$((total_operations + ops))
    done
    
    local test_end=$(date +%s)
    local actual_duration=$((test_end - test_start))
    local ops_per_sec=0
    if [ $actual_duration -gt 0 ]; then
        ops_per_sec=$((total_operations / actual_duration))
    fi
    
    log_info "Stress test results:"
    log_info "  Duration: ${actual_duration}s"
    log_info "  Total operations: $total_operations"
    log_info "  Operations per second: $ops_per_sec"
    
    # Save stress test results
    cat >> "$REPORT_FILE" << EOF
  "stress_test": {
    "duration_seconds": $actual_duration,
    "total_operations": $total_operations,
    "operations_per_second": $ops_per_sec,
    "concurrent_workers": 3
  },
EOF
    
    # Cleanup stress test keys
    for i in {1..3}; do
        for j in {0..1000}; do
            redis-cli -h "$LOAD_BALANCER_HOST" -p "$LOAD_BALANCER_PORT" del "stress_${i}_${j}" > /dev/null 2>&1 || true
        done
    done
    
    # Cleanup
    rm -rf "$temp_dir"
    
    if $success; then
        end_test 0 "Stress Test"
    else
        end_test 1 "Stress Test"
    fi
}

# Generate comprehensive report
generate_report() {
    local end_time=$(date +%s)
    local total_duration=$((end_time - START_TIME))
    local success_rate=0
    if [ $TOTAL_TESTS -gt 0 ]; then
        success_rate=$(( (PASSED_TESTS * 100) / TOTAL_TESTS ))
    fi
    
    log_info "Generating comprehensive test report..."
    
    # Update report with final statistics
    cat >> "$REPORT_FILE" << EOF
  "test_summary": {
    "total_tests": $TOTAL_TESTS,
    "passed_tests": $PASSED_TESTS,
    "failed_tests": $FAILED_TESTS,
    "success_rate_percent": $success_rate
  },
  "duration_info": {
    "total_duration_seconds": $total_duration,
    "start_time": "$(date -u -r $START_TIME +"%Y-%m-%dT%H:%M:%SZ")",
    "end_time": "$(date -u -r $end_time +"%Y-%m-%dT%H:%M:%SZ")"
  }
}
EOF
    
    # Generate human-readable summary
    log_info ""
    log_info "=== TEST EXECUTION SUMMARY ==="
    log_info "Total Tests: $TOTAL_TESTS"
    log_info "Passed: $PASSED_TESTS"
    log_info "Failed: $FAILED_TESTS"
    log_info "Success Rate: ${success_rate}%"
    log_info "Total Duration: ${total_duration}s"
    log_info "Report File: $REPORT_FILE"
    log_info "Log File: $LOG_FILE"
    
    # Final verdict
    if [ $FAILED_TESTS -eq 0 ]; then
        log_success "🎉 ALL TESTS PASSED - Redis load balancer is working correctly!"
    elif [ $FAILED_TESTS -le 2 ]; then
        log_warning "⚠️  MOSTLY PASSED - Redis load balancer is working with minor issues"
    else
        log_error "❌ MULTIPLE FAILURES - Redis load balancer needs attention"
    fi
}

# Cleanup function
cleanup() {
    log_info "Cleaning up test environment..."
    cleanup_test_keys
    log_info "Test cleanup completed"
}

# Main execution
main() {
    init_test
    
    # Run all tests
    test_basic_connectivity
    test_cluster_info
    test_basic_operations
    test_slot_routing
    test_concurrent_connections
    test_performance_benchmark
    test_redis_data_types
    test_error_handling
    test_connection_resilience
    test_stress_test
    
    # Generate report and cleanup
    generate_report
    cleanup
    
    exit $FAILED_TESTS
}

# Handle script interruption
trap cleanup EXIT

# Run main function
main "$@"
exit_code=$?
exit $exit_code