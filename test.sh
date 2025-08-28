#!/bin/bash

# Quick test runner for project root
# This script provides convenient test shortcuts from the project root

echo "🧪 Puerta Test Runner"
echo "======================"
echo ""

# Check if we're in project root
if [ ! -f "Cargo.toml" ] || [ ! -d "tests" ]; then
    echo "❌ Please run this script from the project root directory"
    exit 1
fi

case "${1:-help}" in
    "basic")
        echo "Running basic MongoDB test..."
        ./tests/mongodb/test_mongodb_lb_basic.sh
        ;;
    "quick")
        echo "Running quick MongoDB test..."
        ./tests/mongodb/test_mongodb_lb_quick.sh
        ;;
    "full"|"comprehensive")
        echo "Running comprehensive MongoDB test..."
        ./tests/mongodb/test_mongodb_lb_comprehensive.sh
        ;;
    "redis-basic")
        echo "Running basic Redis test..."
        ./tests/redis/test_redis_lb_basic.sh
        ;;
    "redis-quick")
        echo "Running quick Redis test..."
        ./tests/redis/test_redis_lb_quick.sh
        ;;
    "redis-full"|"redis-comprehensive")
        echo "Running comprehensive Redis test..."
        ./tests/redis/test_redis_lb_comprehensive.sh
        ;;
    "all")
        echo "Running all tests..."
        ./tests/run_tests.sh
        ;;
    "help"|*)
        echo "Usage: $0 [COMMAND]"
        echo ""
        echo "MongoDB Tests:"
        echo "  basic           Basic MongoDB functionality test (~15s)"
        echo "  quick           Quick MongoDB verification test (~30s)"
        echo "  full/comprehensive Comprehensive MongoDB test suite (~2-3min)"
        echo ""
        echo "Redis Tests:"
        echo "  redis-basic     Basic Redis functionality test (~15s)"
        echo "  redis-quick     Quick Redis verification test (~30s)"
        echo "  redis-full      Comprehensive Redis test suite (~2-3min)"
        echo ""
        echo "Combined Tests:"
        echo "  all             Run all tests with validation"
        echo "  help            Show this help message"
        echo ""
        echo "Examples:"
        echo "  $0 basic        # Run MongoDB basic test"
        echo "  $0 redis-basic  # Run Redis basic test"
        echo "  $0 quick        # Run MongoDB quick test"
        echo "  $0 redis-quick  # Run Redis quick test"
        echo "  $0 full         # Run MongoDB comprehensive test"
        echo "  $0 redis-full   # Run Redis comprehensive test"
        echo "  $0 all          # Run all tests"
        ;;
esac