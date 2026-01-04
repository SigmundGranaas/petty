#!/bin/bash
# PDF Service API Test Script
#
# Prerequisites:
#   - PostgreSQL running (see docker-compose.yml or run manually)
#   - pdf-service running (cargo run -p pdf-service)
#
# Usage:
#   ./test-api.sh                    # Run all tests
#   ./test-api.sh health             # Health check only
#   ./test-api.sh sync               # Sync generation test
#   ./test-api.sh async              # Async job test
#   ./test-api.sh load [count]       # Load test with N invoices (default: 10)

set -e

# Configuration
API_BASE="${API_BASE:-http://localhost:3000}"
API_KEY="${API_KEY:-dev-secret-key}"
OUTPUT_DIR="${OUTPUT_DIR:-./test-output}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Create output directory
mkdir -p "$OUTPUT_DIR"

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Health check
test_health() {
    log_info "Testing health endpoint..."
    response=$(curl -s -w "\n%{http_code}" "$API_BASE/health")
    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | sed '$d')

    if [ "$http_code" = "200" ]; then
        log_info "Health check passed: $body"
        return 0
    else
        log_error "Health check failed (HTTP $http_code): $body"
        return 1
    fi
}

# Generate sample invoice data
generate_invoice_data() {
    local invoice_num="${1:-001}"
    cat <<EOF
{
  "invoice": {
    "number": "INV-2025-$invoice_num",
    "company": {
      "name": "Petty PDF Solutions Inc.",
      "address": "123 Document Lane",
      "city": "San Francisco",
      "zip": "94102"
    },
    "customer": {
      "name": "Customer $invoice_num Corp.",
      "email": "billing$invoice_num@example.com"
    },
    "items": [
      {
        "description": "PDF Generation Service - Premium Plan",
        "quantity": 1,
        "price": "299.00",
        "total": "299.00"
      },
      {
        "description": "API Calls (10,000 requests)",
        "quantity": $((RANDOM % 10 + 1)),
        "price": "49.00",
        "total": "$((49 * (RANDOM % 10 + 1))).00"
      }
    ],
    "subtotal": "594.00",
    "tax_rate": "8.5",
    "tax": "50.49",
    "total": "644.49"
  }
}
EOF
}

# Synchronous PDF generation
test_sync_generation() {
    local invoice_num="${1:-001}"
    local output_file="$OUTPUT_DIR/sync-invoice-$invoice_num.pdf"

    log_info "Testing synchronous PDF generation (invoice #$invoice_num)..."

    local invoice_data=$(generate_invoice_data "$invoice_num")
    local request_body=$(cat <<EOF
{
    "template": "invoice",
    "data": $invoice_data
}
EOF
)

    response=$(curl -s -w "\n%{http_code}" \
        -X POST "$API_BASE/api/v1/generate" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: $API_KEY" \
        -d "$request_body" \
        -o "$output_file")

    http_code=$(echo "$response" | tail -n1)

    if [ "$http_code" = "200" ]; then
        file_size=$(stat -c%s "$output_file" 2>/dev/null || stat -f%z "$output_file" 2>/dev/null)
        log_info "Sync generation succeeded: $output_file ($file_size bytes)"
        return 0
    else
        log_error "Sync generation failed (HTTP $http_code)"
        cat "$output_file" 2>/dev/null || true
        rm -f "$output_file"
        return 1
    fi
}

# Asynchronous PDF generation
test_async_generation() {
    local invoice_num="${1:-001}"

    log_info "Testing asynchronous PDF generation (invoice #$invoice_num)..."

    local invoice_data=$(generate_invoice_data "$invoice_num")
    local request_body=$(cat <<EOF
{
    "template": "invoice",
    "data": $invoice_data
}
EOF
)

    # Create job
    response=$(curl -s -w "\n%{http_code}" \
        -X POST "$API_BASE/api/v1/jobs" \
        -H "Content-Type: application/json" \
        -H "X-API-Key: $API_KEY" \
        -d "$request_body")

    http_code=$(echo "$response" | tail -n1)
    body=$(echo "$response" | sed '$d')

    if [ "$http_code" != "202" ]; then
        log_error "Job creation failed (HTTP $http_code): $body"
        return 1
    fi

    job_id=$(echo "$body" | grep -o '"job_id":"[^"]*"' | cut -d'"' -f4)
    log_info "Job created: $job_id"

    # Poll for completion
    local max_attempts=30
    local attempt=0
    local status="pending"

    while [ "$status" = "pending" ] || [ "$status" = "processing" ]; do
        sleep 1
        attempt=$((attempt + 1))

        if [ $attempt -gt $max_attempts ]; then
            log_error "Job timed out after $max_attempts seconds"
            return 1
        fi

        response=$(curl -s -w "\n%{http_code}" \
            -H "X-API-Key: $API_KEY" \
            "$API_BASE/api/v1/jobs/$job_id")

        http_code=$(echo "$response" | tail -n1)
        body=$(echo "$response" | sed '$d')

        if [ "$http_code" != "200" ]; then
            log_error "Status check failed (HTTP $http_code): $body"
            return 1
        fi

        status=$(echo "$body" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)
        log_info "Job status: $status (attempt $attempt)"
    done

    if [ "$status" = "failed" ]; then
        log_error "Job failed: $body"
        return 1
    fi

    # Download result
    local output_file="$OUTPUT_DIR/async-invoice-$invoice_num.pdf"

    response=$(curl -s -w "\n%{http_code}" \
        -H "X-API-Key: $API_KEY" \
        "$API_BASE/api/v1/jobs/$job_id/download" \
        -o "$output_file")

    http_code=$(echo "$response" | tail -n1)

    if [ "$http_code" = "200" ]; then
        file_size=$(stat -c%s "$output_file" 2>/dev/null || stat -f%z "$output_file" 2>/dev/null)
        log_info "Async generation succeeded: $output_file ($file_size bytes)"
        return 0
    else
        log_error "Download failed (HTTP $http_code)"
        cat "$output_file" 2>/dev/null || true
        rm -f "$output_file"
        return 1
    fi
}

# Load test - generate multiple invoices
test_load() {
    local count="${1:-10}"
    local mode="${2:-sync}"

    log_info "Starting load test: $count invoices ($mode mode)"

    local start_time=$(date +%s.%N)
    local success=0
    local failed=0

    for i in $(seq -w 1 "$count"); do
        if [ "$mode" = "async" ]; then
            if test_async_generation "$i" 2>/dev/null; then
                success=$((success + 1))
            else
                failed=$((failed + 1))
            fi
        else
            if test_sync_generation "$i" 2>/dev/null; then
                success=$((success + 1))
            else
                failed=$((failed + 1))
            fi
        fi
    done

    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc)
    local rate=$(echo "scale=2; $count / $duration" | bc)

    echo ""
    log_info "Load test completed:"
    log_info "  Total: $count"
    log_info "  Success: $success"
    log_info "  Failed: $failed"
    log_info "  Duration: ${duration}s"
    log_info "  Rate: ${rate} PDFs/sec"
}

# Parallel load test
test_parallel_load() {
    local count="${1:-50}"
    local concurrency="${2:-5}"

    log_info "Starting parallel load test: $count invoices, $concurrency concurrent"

    local start_time=$(date +%s.%N)
    local pids=()
    local results_file=$(mktemp)

    for i in $(seq 1 "$count"); do
        # Wait if we've reached max concurrency
        while [ ${#pids[@]} -ge "$concurrency" ]; do
            for j in "${!pids[@]}"; do
                if ! kill -0 "${pids[$j]}" 2>/dev/null; then
                    unset 'pids[j]'
                fi
            done
            pids=("${pids[@]}")
            sleep 0.1
        done

        # Start async job in background
        (
            invoice_num=$(printf "%04d" "$i")
            if test_sync_generation "$invoice_num" >/dev/null 2>&1; then
                echo "1" >> "$results_file"
            else
                echo "0" >> "$results_file"
            fi
        ) &
        pids+=($!)
    done

    # Wait for all jobs to complete
    wait

    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc)
    local success=$(grep -c "1" "$results_file" 2>/dev/null || echo "0")
    local failed=$((count - success))
    local rate=$(echo "scale=2; $count / $duration" | bc)

    rm -f "$results_file"

    echo ""
    log_info "Parallel load test completed:"
    log_info "  Total: $count"
    log_info "  Success: $success"
    log_info "  Failed: $failed"
    log_info "  Duration: ${duration}s"
    log_info "  Rate: ${rate} PDFs/sec"
    log_info "  Concurrency: $concurrency"
}

# Main
case "${1:-all}" in
    health)
        test_health
        ;;
    sync)
        test_sync_generation "${2:-001}"
        ;;
    async)
        test_async_generation "${2:-001}"
        ;;
    load)
        test_load "${2:-10}" "${3:-sync}"
        ;;
    parallel)
        test_parallel_load "${2:-50}" "${3:-5}"
        ;;
    all)
        log_info "Running all API tests..."
        echo ""

        if ! test_health; then
            log_error "Health check failed. Is the service running?"
            exit 1
        fi
        echo ""

        test_sync_generation "sync-test"
        echo ""

        test_async_generation "async-test"
        echo ""

        log_info "All tests passed!"
        ;;
    *)
        echo "Usage: $0 {health|sync|async|load [count] [sync|async]|parallel [count] [concurrency]|all}"
        exit 1
        ;;
esac
