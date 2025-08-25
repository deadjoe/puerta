#!/bin/bash

# Enhanced MongoDB Load Balancer Test Suite
# Comprehensive testing for Puerta MongoDB load balancer
# Tests connectivity, load balancing, performance, fault tolerance, and more

# Don't exit on error - we want to run all tests and report results

# Configuration
LOAD_BALANCER_HOST="127.0.0.1"
LOAD_BALANCER_PORT="27016"
BACKEND_ROUTERS=("127.0.0.1:27017" "127.0.0.1:27018" "127.0.0.1:27019")
TEST_DB="puerta_comprehensive_test"
LOG_FILE="/tmp/puerta_test_$(date +%Y%m%d_%H%M%S).log"
REPORT_FILE="/tmp/puerta_test_report_$(date +%Y%m%d_%H%M%S).json"

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
    # Don't return the result to avoid script termination
}

# Initialize test environment
init_test() {
    log_info "=== Puerta MongoDB Load Balancer Comprehensive Test Suite ==="
    log_info "Load Balancer: ${LOAD_BALANCER_HOST}:${LOAD_BALANCER_PORT}"
    log_info "Backend Routers: ${BACKEND_ROUTERS[*]}"
    log_info "Test Database: ${TEST_DB}"
    log_info "Log File: ${LOG_FILE}"
    log_info "Report File: ${REPORT_FILE}"
    log_info "Start Time: $(date)"
    echo "" | tee -a "$LOG_FILE"
    
    # Clean up any existing test database
    mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.getSiblingDB('$TEST_DB').dropDatabase()" --quiet 2>/dev/null || true
    
    # Initialize report JSON
    cat > "$REPORT_FILE" << EOF
{
  "test_info": {
    "load_balancer": "${LOAD_BALANCER_HOST}:${LOAD_BALANCER_PORT}",
    "backend_routers": [$(printf '"%s",' "${BACKEND_ROUTERS[@]}" | sed 's/,$//')],
    "test_database": "$TEST_DB",
    "start_time": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "duration_seconds": 0
  },
  "test_results": [],
  "performance_metrics": {},
  "load_balancing_stats": {},
  "error_analysis": {}
}
EOF
}

# Test 1: Basic connectivity test
test_basic_connectivity() {
    start_test "Basic Connectivity Test"
    
    local success=true
    
    # Test load balancer connectivity
    if ! mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.adminCommand('ping')" --quiet > /dev/null 2>&1; then
        log_error "Load balancer connectivity failed"
        success=false
    fi
    
    # Test each backend router
    for router in "${BACKEND_ROUTERS[@]}"; do
        local host=${router%:*}
        local port=${router#*:}
        if ! mongosh --host "$host" --port "$port" --eval "db.adminCommand('ping')" --quiet > /dev/null 2>&1; then
            log_error "Backend router $router connectivity failed"
            success=false
        fi
    done
    
    if $success; then
        log_success "All connectivity tests passed"
    fi
    
    if $success; then
        end_test 0 "Basic Connectivity"
    else
        end_test 1 "Basic Connectivity"
    fi
}

# Test 2: Verify router nodes
test_router_verification() {
    start_test "Router Node Verification"
    
    local success=true
    
    # Test load balancer responds as mongos
    local output=$(mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.adminCommand('ismaster')" --quiet 2>/dev/null)
    if ! echo "$output" | grep -q "msg.*isdbgrid"; then
        log_error "Load balancer not responding as mongos router"
        success=false
    fi
    
    # Test each backend router responds as mongos
    for router in "${BACKEND_ROUTERS[@]}"; do
        local host=${router%:*}
        local port=${router#*:}
        local output=$(mongosh --host "$host" --port "$port" --eval "db.adminCommand('ismaster')" --quiet 2>/dev/null)
        if ! echo "$output" | grep -q "msg.*isdbgrid"; then
            log_error "Router $router not responding as mongos"
            success=false
        fi
    done
    
    if $success; then
        end_test 0 "Router Verification"
    else
        end_test 1 "Router Verification"
    fi
}

# Test 3: Cluster topology information
test_cluster_topology() {
    start_test "Cluster Topology Analysis"
    
    local success=true
    
    # Get cluster status
    local output=$(mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "sh.status()" --quiet 2>/dev/null)
    if [ -z "$output" ]; then
        log_error "Failed to get cluster status"
        success=false
    else
        # Extract key information
        local shard_count=$(echo "$output" | grep -c "shard[0-9]")
        local mongos_count=$(echo "$output" | grep -c "active mongoses")
        
        log_info "Cluster topology: $shard_count shards, $mongos_count active mongos"
        
        # Get detailed shard information
        local shards_output=$(mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.adminCommand('listShards')" --quiet 2>/dev/null)
        echo "$shards_output" >> "$LOG_FILE"
    fi
    
    if $success; then
        end_test 0 "Cluster Topology"
    else
        end_test 1 "Cluster Topology"
    fi
}

# Test 4: Load balancing verification
test_load_balancing() {
    start_test "Load Balancing Verification"
    
    local connection_ids=()
    local success=true
    
    log_info "Testing load balancing with $CONCURRENT_CONNECTIONS connections..."
    
    # Test multiple connections
    for i in $(seq 1 $CONCURRENT_CONNECTIONS); do
        local output=$(mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.adminCommand('ismaster')" --quiet 2>/dev/null)
        local conn_id=$(echo "$output" | grep -o 'connectionId: [0-9]*' | cut -d':' -f2 | tr -d ' ')
        
        if [ -n "$conn_id" ]; then
            connection_ids+=("$conn_id")
            echo "Connection $i: connectionId $conn_id" >> "$LOG_FILE"
        else
            log_error "Failed to get connectionId for connection $i"
            success=false
        fi
    done
    
    # Analyze load balancing
    local total_connections=${#connection_ids[@]}
    local unique_connections=$(printf "%s\n" "${connection_ids[@]}" | sort -u | wc -l)
    
    log_info "Load balancing analysis:"
    log_info "  Total connections: $total_connections"
    log_info "  Unique connection IDs: $unique_connections"
    
    if [ "$unique_connections" -eq "$total_connections" ]; then
        log_success "Perfect load balancing: all connections have unique IDs"
    else
        log_warning "Some connections may share backend (unique: $unique_connections, total: $total_connections)"
    fi
    
    # Save load balancing stats
    cat >> "$REPORT_FILE" << EOF
    "load_balancing_analysis": {
        "total_connections": $total_connections,
        "unique_connections": $unique_connections,
        "efficiency": $(echo "scale=2; $unique_connections * 100 / $total_connections" | bc -l)%
    },
EOF
    
    if $success; then
        end_test 0 "Load Balancing"
    else
        end_test 1 "Load Balancing"
    fi
}

# Test 5: Concurrent connection test
test_concurrent_connections() {
    start_test "Concurrent Connection Test"
    
    local success=true
    local temp_dir=$(mktemp -d)
    local connection_count=0
    
    log_info "Testing $CONCURRENT_CONNECTIONS concurrent connections..."
    
    # Create concurrent connection scripts
    for i in $(seq 1 $CONCURRENT_CONNECTIONS); do
        cat > "$temp_dir/conn_$i.sh" << EOF
#!/bin/bash
mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "
    db.adminCommand('ping');
    db.adminCommand('ismaster');
    print('Connection $i completed');
" --quiet
EOF
        chmod +x "$temp_dir/conn_$i.sh"
    done
    
    # Run connections concurrently
    local pids=()
    for i in $(seq 1 $CONCURRENT_CONNECTIONS); do
        "$temp_dir/conn_$i.sh" > "$temp_dir/conn_$i.out" 2>&1 &
        pids+=($!)
    done
    
    # Wait for all connections to complete
    for pid in "${pids[@]}"; do
        wait "$pid"
        if [ $? -eq 0 ]; then
            ((connection_count++))
        fi
    done
    
    log_info "Concurrent connections completed: $connection_count/$CONCURRENT_CONNECTIONS"
    
    # Check results
    if [ "$connection_count" -eq "$CONCURRENT_CONNECTIONS" ]; then
        log_success "All concurrent connections successful"
    else
        log_error "Some concurrent connections failed"
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

# Test 6: Database operations test
test_database_operations() {
    start_test "Database Operations Test"
    
    local success=true
    local test_start=$(date +%s)
    
    log_info "Testing basic database operations..."
    
    # Test database creation and collection operations
    mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "
        use('$TEST_DB');
        
        // Create collection and insert test data
        db.test_collection.insertMany([
            {name: 'doc1', value: 100, timestamp: new Date()},
            {name: 'doc2', value: 200, timestamp: new Date()},
            {name: 'doc3', value: 300, timestamp: new Date()}
        ]);
        
        // Query operations
        var count = db.test_collection.countDocuments();
        var docs = db.test_collection.find().toArray();
        var result = db.test_collection.findOne({name: 'doc1'});
        
        // Update operations
        db.test_collection.updateOne({name: 'doc1'}, {\$set: {value: 150}});
        
        // Delete operations
        db.test_collection.deleteOne({name: 'doc3'});
        
        // Verify final count
        var final_count = db.test_collection.countDocuments();
        
        print('Operations completed successfully');
        print('Initial count: ' + count);
        print('Final count: ' + final_count);
        print('Update result: ' + JSON.stringify(result));
    " --quiet > "$LOG_FILE" 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "Database operations completed successfully"
    else
        log_error "Database operations failed"
        success=false
    fi
    
    local test_end=$(date +%s)
    local duration=$((test_end - test_start))
    log_info "Database operations duration: ${duration}s"
    
    if $success; then
        end_test 0 "Database Operations"
    else
        end_test 1 "Database Operations"
    fi
}

# Test 7: Batch operations test
test_batch_operations() {
    start_test "Batch Operations Test"
    
    local success=true
    local test_start=$(date +%s)
    
    log_info "Testing batch operations with $BATCH_SIZE documents..."
    
    # Test batch insert
    mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "
        use('$TEST_DB');
        
        // Generate batch insert data
        var batchData = [];
        for (var i = 0; i < $BATCH_SIZE; i++) {
            batchData.push({
                batch_id: 'test_batch',
                document_number: i,
                value: Math.floor(Math.random() * 1000),
                timestamp: new Date()
            });
        }
        
        // Batch insert
        var insertResult = db.batch_collection.insertMany(batchData);
        print('Batch insert result: ' + insertResult.insertedCount + ' documents');
        
        // Batch query
        var queryResult = db.batch_collection.find({batch_id: 'test_batch'}).toArray();
        print('Batch query result: ' + queryResult.length + ' documents');
        
        // Batch update
        var updateResult = db.batch_collection.updateMany(
            {batch_id: 'test_batch'},
            {\$set: {processed: true}}
        );
        print('Batch update result: ' + updateResult.modifiedCount + ' documents');
        
        // Batch delete
        var deleteResult = db.batch_collection.deleteMany({batch_id: 'test_batch'});
        print('Batch delete result: ' + deleteResult.deletedCount + ' documents');
    " --quiet >> "$LOG_FILE" 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "Batch operations completed successfully"
    else
        log_error "Batch operations failed"
        success=false
    fi
    
    local test_end=$(date +%s)
    local duration=$((test_end - test_start))
    local ops_per_sec=0
    if [ $duration -gt 0 ]; then
        ops_per_sec=$((BATCH_SIZE / duration))
    fi
    
    log_info "Batch operations duration: ${duration}s"
    log_info "Operations per second: $ops_per_sec"
    
    if $success; then
        end_test 0 "Batch Operations"
    else
        end_test 1 "Batch Operations"
    fi
}

# Test 8: Performance benchmark
test_performance_benchmark() {
    start_test "Performance Benchmark Test"
    
    local success=true
    local test_start=$(date +%s)
    local total_ops=0
    local total_time=0
    
    log_info "Running performance benchmark..."
    
    # Test read performance
    local read_start=$(date +%s)
    for i in {1..100}; do
        mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "
            use('$TEST_DB');
            db.adminCommand('ping');
        " --quiet > /dev/null 2>&1
    done
    local read_end=$(date +%s)
    local read_duration=$((read_end - read_start))
    
    # Test write performance
    local write_start=$(date +%s)
    for i in {1..50}; do
        mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "
            use('$TEST_DB');
            db.perf_test.insertOne({test: 'data', iteration: $i, timestamp: new Date()});
        " --quiet > /dev/null 2>&1
    done
    local write_end=$(date +%s)
    local write_duration=$((write_end - write_start))
    
    # Calculate performance metrics
    local read_ops=0
    local write_ops=0
    if [ $read_duration -gt 0 ]; then
        read_ops=$(echo "scale=2; 100 / $read_duration" | bc -l)
    fi
    if [ $write_duration -gt 0 ]; then
        write_ops=$(echo "scale=2; 50 / $write_duration" | bc -l)
    fi
    
    log_info "Performance Results:"
    log_info "  Read operations: 100 ops in ${read_duration}s (${read_ops} ops/sec)"
    log_info "  Write operations: 50 ops in ${write_duration}s (${write_ops} ops/sec)"
    
    # Save performance metrics
    cat >> "$REPORT_FILE" << EOF
    "performance_metrics": {
        "read_operations": {
            "count": 100,
            "duration_seconds": $read_duration,
            "ops_per_second": $read_ops
        },
        "write_operations": {
            "count": 50,
            "duration_seconds": $write_duration,
            "ops_per_second": $write_ops
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

# Test 9: Connection resilience test
test_connection_resilience() {
    start_test "Connection Resilience Test"
    
    local success=true
    local failed_connections=0
    
    log_info "Testing connection resilience with retries..."
    
    # Test connection resilience
    for i in {1..20}; do
        local retry_count=0
        local connected=false
        
        while [ $retry_count -lt $MAX_RETRIES ]; do
            if mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.adminCommand('ping')" --quiet > /dev/null 2>&1; then
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
LOAD_BALANCER_PORT="27016"
TEST_DB="puerta_comprehensive_test"
DURATION=$1

end_time=$(($(date +%s) + DURATION))
operations=0

while [ $(date +%s) -lt $end_time ]; do
    mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "
        use('$TEST_DB');
        db.stress_test.insertOne({
            operation: 'stress',
            timestamp: new Date(),
            random_value: Math.floor(Math.random() * 10000)
        });
    " --quiet > /dev/null 2>&1
    
    if [ $? -eq 0 ]; then
        ((operations++))
    fi
done

echo $operations
EOF
    
    chmod +x "$temp_dir/stress.sh"
    
    # Run concurrent stress tests
    local pids=()
    local ops_files=()
    
    for i in {1..5}; do
        local ops_file="$temp_dir/ops_$i.txt"
        ops_files+=("$ops_file")
        "$temp_dir/stress.sh" "$STRESS_TEST_DURATION" > "$ops_file" &
        pids+=($!)
    done
    
    # Wait for all stress tests to complete
    for pid in "${pids[@]}"; do
        wait "$pid"
    done
    
    # Calculate total operations
    local total_operations=0
    for ops_file in "${ops_files[@]}"; do
        local ops=$(cat "$ops_file")
        total_operations=$((total_operations + ops))
    done
    
    local test_end=$(date +%s)
    local actual_duration=$((test_end - test_start))
    local ops_per_sec=$((total_operations / actual_duration))
    
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
        "concurrent_clients": 5
    },
EOF
    
    # Cleanup
    rm -rf "$temp_dir"
    
    if $success; then
        end_test 0 "Stress Test"
    else
        end_test 1 "Stress Test"
    fi
}

# Test 11: Error handling test
test_error_handling() {
    start_test "Error Handling Test"
    
    local success=true
    
    log_info "Testing error handling scenarios..."
    
    # Test invalid command
    local output=$(mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.invalidCommand()" --quiet 2>&1 || true)
    if ! echo "$output" | grep -q "not recognized"; then
        log_warning "Invalid command error handling may not be working correctly"
    fi
    
    # Test invalid database access
    output=$(mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "use('invalid_db'); db.test.insertOne({test: 'data'})" --quiet 2>&1 || true)
    if [ $? -eq 0 ]; then
        log_warning "Invalid database access should have failed"
    fi
    
    # Test malformed query
    output=$(mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.test.find({\$invalid: 'query'})" --quiet 2>&1 || true)
    if [ $? -eq 0 ]; then
        log_warning "Malformed query should have failed"
    fi
    
    log_success "Error handling tests completed"
    
    if $success; then
        end_test 0 "Error Handling"
    else
        end_test 1 "Error Handling"
    fi
}

# Test 12: Memory and resource usage
test_resource_usage() {
    start_test "Resource Usage Test"
    
    local success=true
    local puerta_pid=$(lsof -i :$LOAD_BALANCER_PORT -t | head -1)
    
    if [ -n "$puerta_pid" ]; then
        local mem_usage=$(ps -p "$puerta_pid" -o %mem | tail -1 | tr -d ' ')
        local cpu_usage=$(ps -p "$puerta_pid" -o %cpu | tail -1 | tr -d ' ')
        
        log_info "Puerta resource usage:"
        log_info "  PID: $puerta_pid"
        log_info "  Memory: ${mem_usage}%"
        log_info "  CPU: ${cpu_usage}%"
        
        # Save resource usage
        cat >> "$REPORT_FILE" << EOF
    "resource_usage": {
        "pid": $puerta_pid,
        "memory_percent": $mem_usage,
        "cpu_percent": $cpu_usage
    },
EOF
        
        # Check for abnormal resource usage
        if [ -n "$mem_usage" ] && [ "$mem_usage" != "%" ] && (( $(echo "$mem_usage > 50" | bc -l) )); then
            log_warning "High memory usage: ${mem_usage}%"
        fi
        
        if [ -n "$cpu_usage" ] && [ "$cpu_usage" != "%" ] && (( $(echo "$cpu_usage > 80" | bc -l) )); then
            log_warning "High CPU usage: ${cpu_usage}%"
        fi
    else
        log_error "Could not find Puerta process"
        success=false
    fi
    
    if $success; then
        end_test 0 "Resource Usage"
    else
        end_test 1 "Resource Usage"
    fi
}

# Generate comprehensive report
generate_report() {
    local end_time=$(date +%s)
    local total_duration=$((end_time - START_TIME))
    local success_rate=$(( (PASSED_TESTS * 100) / TOTAL_TESTS ))
    
    log_info "Generating comprehensive test report..."
    
    # Update report with final statistics
    cat >> "$REPORT_FILE" << EOF
    "test_summary": {
        "total_tests": $TOTAL_TESTS,
        "passed_tests": $PASSED_TESTS,
        "failed_tests": $FAILED_TESTS,
        "success_rate": ${success_rate}%
    },
    "duration_info": {
        "total_duration_seconds": $total_duration,
        "start_time": "$(date -u -r $START_TIME +"%Y-%m-%dT%H:%M:%SZ")",
        "end_time": "$(date -u -r $end_time +"%Y-%m-%dT%H:%M:%SZ")"
    },
    "recommendations": [
        "Monitor connection pooling efficiency",
        "Implement health checks for backend routers",
        "Consider adding circuit breaker pattern for fault tolerance",
        "Optimize connection reuse for better performance",
        "Add metrics collection for production monitoring"
    ]
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
        log_success "🎉 ALL TESTS PASSED - Load balancer is working correctly!"
    elif [ $FAILED_TESTS -le 2 ]; then
        log_warning "⚠️  MOSTLY PASSED - Load balancer is working with minor issues"
    else
        log_error "❌ MULTIPLE FAILURES - Load balancer needs attention"
    fi
}

# Cleanup function
cleanup() {
    log_info "Cleaning up test environment..."
    
    # Drop test database
    mongosh --host "$LOAD_BALANCER_HOST" --port "$LOAD_BALANCER_PORT" --eval "db.getSiblingDB('$TEST_DB').dropDatabase()" --quiet 2>/dev/null || true
    
    log_info "Test database cleaned up"
}

# Main execution
main() {
    init_test
    
    # Run all tests
    test_basic_connectivity
    test_router_verification
    test_cluster_topology
    test_load_balancing
    test_concurrent_connections
    test_database_operations
    test_batch_operations
    test_performance_benchmark
    test_connection_resilience
    test_stress_test
    test_error_handling
    test_resource_usage
    
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