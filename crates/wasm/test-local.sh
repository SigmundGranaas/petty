#!/bin/bash
# Local test script for WASM module
# Run from repository root: ./crates/wasm/test-local.sh

set -e  # Exit on error

echo "========================================="
echo "Petty WASM Local Test Script"
echo "========================================="
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Please run this script from the repository root${NC}"
    exit 1
fi

# Step 1: Check for wasm-pack
echo -e "${YELLOW}[1/6]${NC} Checking for wasm-pack..."
if ! command -v wasm-pack &> /dev/null; then
    echo -e "${RED}wasm-pack not found. Installing...${NC}"
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi
echo -e "${GREEN}✓${NC} wasm-pack found"
echo ""

# Step 2: Check for Node.js
echo -e "${YELLOW}[2/6]${NC} Checking for Node.js..."
if ! command -v node &> /dev/null; then
    echo -e "${RED}Error: Node.js not found. Please install Node.js 20+${NC}"
    exit 1
fi
NODE_VERSION=$(node --version)
echo -e "${GREEN}✓${NC} Node.js found: $NODE_VERSION"
echo ""

# Step 3: Build WASM module
echo -e "${YELLOW}[3/6]${NC} Building WASM module..."
wasm-pack build crates/wasm --target nodejs --release
echo -e "${GREEN}✓${NC} WASM module built"
echo ""

# Step 4: Install test dependencies
echo -e "${YELLOW}[4/6]${NC} Installing test dependencies..."
cd crates/wasm/integration-tests
npm install
echo -e "${GREEN}✓${NC} Dependencies installed"
echo ""

# Step 5: Run tests
echo -e "${YELLOW}[5/6]${NC} Running integration tests..."
echo ""
npm test
TEST_RESULT=$?
echo ""

# Step 6: Summary
if [ $TEST_RESULT -eq 0 ]; then
    echo -e "${GREEN}========================================="
    echo "All tests passed! ✓"
    echo "=========================================${NC}"
    echo ""
    echo "Generated PDFs can be found in:"
    echo "  crates/wasm/integration-tests/output/"
    echo ""
    echo "To run tests again:"
    echo "  cd crates/wasm/integration-tests && npm test"
else
    echo -e "${RED}========================================="
    echo "Some tests failed ✗"
    echo "=========================================${NC}"
    exit 1
fi
