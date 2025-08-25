#!/bin/bash

# Puerta Test Runner
# Main test runner for all Puerta load balancer tests

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Puerta Load Balancer Test Suite ==="
echo "Project directory: $PROJECT_DIR"
echo "Test directory: $SCRIPT_DIR"
echo ""

# Check if load balancer is running
echo "Checking if Puerta load balancer is running..."
if ! lsof -i :27016 | grep -q "LISTEN"; then
    echo "❌ Puerta load balancer not found on port 27016"
    echo "Please start the load balancer before running tests"
    exit 1
fi
echo "✅ Puerta load balancer is running"

# Check MongoDB connectivity
echo ""
echo "Checking MongoDB connectivity..."
if ! mongosh --host 127.0.0.1 --port 27017 --eval "db.adminCommand('ping')" --quiet > /dev/null 2>&1; then
    echo "❌ MongoDB cluster not accessible"
    echo "Please ensure MongoDB cluster is running"
    exit 1
fi
echo "✅ MongoDB cluster is accessible"

# Function to run a test
run_test() {
    local test_script="$1"
    local test_name="$2"
    
    echo ""
    echo "Running $test_name..."
    echo "===================================="
    
    if [ -f "$test_script" ]; then
        if bash "$test_script"; then
            echo "✅ $test_name: PASSED"
            return 0
        else
            echo "❌ $test_name: FAILED"
            return 1
        fi
    else
        echo "❌ Test script not found: $test_script"
        return 1
    fi
}

# Parse command line arguments
RUN_BASIC=false
RUN_QUICK=false
RUN_COMPREHENSIVE=false
RUN_ALL=true

while [[ $# -gt 0 ]]; do
    case $1 in
        --basic)
            RUN_BASIC=true
            RUN_ALL=false
            shift
            ;;
        --quick)
            RUN_QUICK=true
            RUN_ALL=false
            shift
            ;;
        --comprehensive)
            RUN_COMPREHENSIVE=true
            RUN_ALL=false
            shift
            ;;
        --all)
            RUN_ALL=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --basic           Run basic functionality test only"
            echo "  --quick           Run quick verification test only"
            echo "  --comprehensive   Run comprehensive test suite only"
            echo "  --all             Run all tests (default)"
            echo "  --help, -h        Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                # Run all tests"
            echo "  $0 --basic       # Run basic test only"
            echo "  $0 --quick       # Run quick test only"
            echo "  $0 --comprehensive # Run comprehensive test only"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Change to project directory
cd "$PROJECT_DIR"

# Run tests based on selection
FAILED_TESTS=0
TOTAL_TESTS=0

if [ "$RUN_ALL" = true ] || [ "$RUN_BASIC" = true ]; then
    ((TOTAL_TESTS++))
    if run_test "$SCRIPT_DIR/mongodb/test_mongodb_lb_basic.sh" "Basic Functionality Test"; then
        echo "✅ Basic test passed"
    else
        ((FAILED_TESTS++))
        echo "❌ Basic test failed"
    fi
fi

if [ "$RUN_ALL" = true ] || [ "$RUN_QUICK" = true ]; then
    ((TOTAL_TESTS++))
    if run_test "$SCRIPT_DIR/mongodb/test_mongodb_lb_quick.sh" "Quick Verification Test"; then
        echo "✅ Quick test passed"
    else
        ((FAILED_TESTS++))
        echo "❌ Quick test failed"
    fi
fi

if [ "$RUN_ALL" = true ] || [ "$RUN_COMPREHENSIVE" = true ]; then
    ((TOTAL_TESTS++))
    if run_test "$SCRIPT_DIR/mongodb/test_mongodb_lb_comprehensive.sh" "Comprehensive Test Suite"; then
        echo "✅ Comprehensive test passed"
    else
        ((FAILED_TESTS++))
        echo "❌ Comprehensive test failed"
    fi
fi

# Summary
echo ""
echo "=== Test Summary ==="
echo "Total tests run: $TOTAL_TESTS"
echo "Failed tests: $FAILED_TESTS"
echo "Passed tests: $((TOTAL_TESTS - FAILED_TESTS))"

if [ $FAILED_TESTS -eq 0 ]; then
    echo "🎉 ALL TESTS PASSED"
    exit 0
else
    echo "❌ $FAILED_TESTS test(s) failed"
    exit 1
fi