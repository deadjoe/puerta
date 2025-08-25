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
        echo "Running basic test..."
        ./tests/mongodb/test_mongodb_lb_basic.sh
        ;;
    "quick")
        echo "Running quick test..."
        ./tests/mongodb/test_mongodb_lb_quick.sh
        ;;
    "full"|"comprehensive")
        echo "Running comprehensive test..."
        ./tests/mongodb/test_mongodb_lb_comprehensive.sh
        ;;
    "all")
        echo "Running all tests..."
        ./tests/run_tests.sh
        ;;
    "help"|*)
        echo "Usage: $0 [COMMAND]"
        echo ""
        echo "Commands:"
        echo "  basic           Basic functionality test (~15s)"
        echo "  quick           Quick verification test (~30s)"
        echo "  full/comprehensive Comprehensive test suite (~2-3min)"
        echo "  all             Run all tests with validation"
        echo "  help            Show this help message"
        echo ""
        echo "Examples:"
        echo "  $0 basic     # Run basic test"
        echo "  $0 quick     # Run quick test"
        echo "  $0 full      # Run comprehensive test"
        echo "  $0 all       # Run all tests"
        ;;
esac