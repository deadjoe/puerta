#!/bin/bash

# Puerta Test Runner
# Convenient test shortcuts for MongoDB and Redis cluster testing

echo "🧪 Puerta Test Runner"
echo "======================"
echo ""

# Check if we're in the tests directory
if [ ! -f "../Cargo.toml" ] || [ ! -d "mongodb" ] || [ ! -d "redis" ]; then
    echo "❌ Please run this script from the tests directory"
    exit 1
fi

# Parse command line arguments
DATABASE="${1:-help}"
TEST_TYPE="${2:-basic}"

case "$DATABASE" in
    "mongo")
        case "$TEST_TYPE" in
            "basic")
                echo "Running MongoDB basic test..."
                ./mongodb/test_mongodb_lb_basic.sh
                ;;
            "quick")
                echo "Running MongoDB quick test..."
                ./mongodb/test_mongodb_lb_quick.sh
                ;;
            "full"|"comprehensive")
                echo "Running MongoDB comprehensive test..."
                ./mongodb/test_mongodb_lb_comprehensive.sh
                ;;
            *)
                echo "❌ Invalid MongoDB test type: $TEST_TYPE"
                echo "Valid options: basic, quick, full"
                exit 1
                ;;
        esac
        ;;
    "redis")
        case "$TEST_TYPE" in
            "basic")
                echo "Running Redis basic test..."
                ./redis/test_redis_lb_basic.sh
                ;;
            "quick")
                echo "Running Redis quick test..."
                ./redis/test_redis_lb_quick.sh
                ;;
            "full"|"comprehensive")
                echo "Running Redis comprehensive test..."
                ./redis/test_redis_lb_comprehensive.sh
                ;;
            *)
                echo "❌ Invalid Redis test type: $TEST_TYPE"
                echo "Valid options: basic, quick, full"
                exit 1
                ;;
        esac
        ;;
    "all")
        echo "Running all tests (MongoDB + Redis)..."
        echo ""
        
        # MongoDB Tests
        echo "=== MongoDB Tests ==="
        echo "Running MongoDB basic test..."
        ./mongodb/test_mongodb_lb_basic.sh
        mongodb_basic_result=$?
        
        echo ""
        echo "Running MongoDB quick test..."
        ./mongodb/test_mongodb_lb_quick.sh
        mongodb_quick_result=$?
        
        echo ""
        echo "Running MongoDB comprehensive test..."
        ./mongodb/test_mongodb_lb_comprehensive.sh
        mongodb_comprehensive_result=$?
        
        # Redis Tests  
        echo ""
        echo "=== Redis Tests ==="
        echo "Running Redis basic test..."
        ./redis/test_redis_lb_basic.sh
        redis_basic_result=$?
        
        echo ""
        echo "Running Redis quick test..."
        ./redis/test_redis_lb_quick.sh
        redis_quick_result=$?
        
        echo ""
        echo "Running Redis comprehensive test..."
        ./redis/test_redis_lb_comprehensive.sh
        redis_comprehensive_result=$?
        
        # Summary
        echo ""
        echo "=== Test Suite Summary ==="
        total_tests=6
        failed_tests=0
        
        # Count failures
        [ $mongodb_basic_result -ne 0 ] && ((failed_tests++)) && echo "❌ MongoDB Basic Test: FAILED"
        [ $mongodb_basic_result -eq 0 ] && echo "✅ MongoDB Basic Test: PASSED"
        
        [ $mongodb_quick_result -ne 0 ] && ((failed_tests++)) && echo "❌ MongoDB Quick Test: FAILED"
        [ $mongodb_quick_result -eq 0 ] && echo "✅ MongoDB Quick Test: PASSED"
        
        [ $mongodb_comprehensive_result -ne 0 ] && ((failed_tests++)) && echo "❌ MongoDB Comprehensive Test: FAILED"
        [ $mongodb_comprehensive_result -eq 0 ] && echo "✅ MongoDB Comprehensive Test: PASSED"
        
        [ $redis_basic_result -ne 0 ] && ((failed_tests++)) && echo "❌ Redis Basic Test: FAILED"
        [ $redis_basic_result -eq 0 ] && echo "✅ Redis Basic Test: PASSED"
        
        [ $redis_quick_result -ne 0 ] && ((failed_tests++)) && echo "❌ Redis Quick Test: FAILED"
        [ $redis_quick_result -eq 0 ] && echo "✅ Redis Quick Test: PASSED"
        
        [ $redis_comprehensive_result -ne 0 ] && ((failed_tests++)) && echo "❌ Redis Comprehensive Test: FAILED"
        [ $redis_comprehensive_result -eq 0 ] && echo "✅ Redis Comprehensive Test: PASSED"
        
        echo ""
        echo "Total tests: $total_tests"
        echo "Passed: $((total_tests - failed_tests))"
        echo "Failed: $failed_tests"
        
        if [ $failed_tests -eq 0 ]; then
            echo ""
            echo "🎉 ALL TESTS PASSED!"
            exit 0
        else
            echo ""
            echo "❌ $failed_tests test(s) failed"
            exit 1
        fi
        ;;
    "help"|*)
        echo "Usage: $0 <database> <test_type>"
        echo "       $0 all"
        echo "       $0 help"
        echo ""
        echo "Database Options:"
        echo "  mongo           MongoDB cluster tests"
        echo "  redis           Redis cluster tests"
        echo "  all             Run all tests (MongoDB + Redis)"
        echo ""
        echo "Test Types:"
        echo "  basic           Basic functionality test (~15s)"
        echo "  quick           Quick verification test (~30s)"
        echo "  full            Comprehensive test suite (~2-3min)"
        echo ""
        echo "Examples:"
        echo "  $0 mongo basic     # MongoDB basic test"
        echo "  $0 redis basic     # Redis basic test"
        echo "  $0 mongo quick     # MongoDB quick test"
        echo "  $0 redis quick     # Redis quick test"
        echo "  $0 mongo full      # MongoDB comprehensive test"
        echo "  $0 redis full      # Redis comprehensive test"
        echo "  $0 all             # Run all tests"
        ;;
esac