#!/bin/bash
# Generate code coverage report using tarpaulin
# Requires: cargo install cargo-tarpaulin

set -e

COVERAGE_DIR="coverage"
THRESHOLD=${1:-85}

echo "📊 Generating code coverage report..."
echo ""

# Check if tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "❌ cargo-tarpaulin is not installed."
    echo "Install it with: cargo install cargo-tarpaulin"
    exit 1
fi

# Create coverage directory
mkdir -p "$COVERAGE_DIR"

# Generate coverage reports
cargo coverage

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
    echo "1. Open coverage/tarpaulin-report.html in your browser"
    echo "2. Look for red-highlighted lines (uncovered code)"
    echo "3. Add tests for those code paths"
    echo "4. Re-run this script to verify improvement"
    exit 1
else
    echo "✅ Coverage ${COVERAGE}% meets threshold ${THRESHOLD}%"
    echo ""
    echo "📁 Coverage reports generated:"
    echo "   • HTML: $COVERAGE_DIR/tarpaulin-report.html"
    echo "   • XML:  $COVERAGE_DIR/cobertura.xml"
    echo ""
    echo "Open the HTML report in your browser to see detailed coverage."
fi
