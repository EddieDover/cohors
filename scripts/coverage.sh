#!/bin/bash
# Generate code coverage report using llvm-cov
# Requires: cargo install cargo-llvm-cov

set -e

COVERAGE_DIR="coverage"
THRESHOLD=${1:-85}

echo "📊 Generating code coverage report..."
echo ""

# Check if llvm-cov is installed
if ! cargo llvm-cov --version &> /dev/null; then
    echo "❌ cargo-llvm-cov is not installed."
    echo "Install it with: cargo install cargo-llvm-cov"
    exit 1
fi

# Create coverage directory
mkdir -p "$COVERAGE_DIR"

# Generate coverage reports
cargo llvm-cov --workspace --all-features --ignore-filename-regex "src/main.rs|src/tests.rs|src/.*/tests.rs|packaging/.*" --html --output-dir coverage
cargo llvm-cov report --lcov --output-path coverage/lcov.info
cargo llvm-cov report --cobertura --output-path coverage/cobertura.xml

# Extract coverage percentage
COVERAGE=$(grep -oP 'line-rate="\K[^"]+' "$COVERAGE_DIR/cobertura.xml" | head -1 | awk '{print int($1*100)}')

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📈 Coverage Report"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Overall Coverage: ${COVERAGE}%"
echo "Threshold: ${THRESHOLD}%"
echo ""

if [ "$COVERAGE" -lt "$THRESHOLD" ]; then
    echo "❌ Coverage ${COVERAGE}% is BELOW threshold ${THRESHOLD}%"
    echo ""
    echo "To improve coverage:"
    echo "1. Open coverage/html/index.html in your browser"
    echo "2. Look for red-highlighted lines (uncovered code)"
    echo "3. Add tests for those code paths"
    echo "4. Re-run this script to verify improvement"
    exit 1
else
    echo "✅ Coverage ${COVERAGE}% meets threshold ${THRESHOLD}%"
    echo ""
    echo "📁 Coverage reports generated:"
    echo "   • HTML: $COVERAGE_DIR/html/index.html"
    echo "   • XML:  $COVERAGE_DIR/cobertura.xml"
    echo ""
    echo "Open the HTML report in your browser to see detailed coverage."
fi
