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
    "help"|*)
        echo "Usage: $0 <database> <test_type>"
        echo "       $0 help"
        echo ""
        echo "Database Options:"
        echo "  mongo           MongoDB cluster tests"
        echo "  redis           Redis cluster tests"
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
        ;;
esac