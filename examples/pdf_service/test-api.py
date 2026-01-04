#!/usr/bin/env python3
"""
PDF Service API Test Script

A Python alternative to test-api.sh for testing the PDF Service.

Usage:
    python test-api.py                        # Run all tests
    python test-api.py --health               # Health check only
    python test-api.py --sync                 # Sync generation test
    python test-api.py --async                # Async job test
    python test-api.py --load 10              # Generate 10 PDFs
    python test-api.py --load 50 --parallel 5 # 50 PDFs, 5 concurrent

Prerequisites:
    pip install requests
"""

import argparse
import json
import os
import random
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

try:
    import requests
except ImportError:
    print("Error: requests library required. Install with: pip install requests")
    sys.exit(1)


# Configuration
API_BASE = os.getenv("API_BASE", "http://localhost:3000")
API_KEY = os.getenv("API_KEY", "dev-secret-key")
OUTPUT_DIR = Path(os.getenv("OUTPUT_DIR", "./test-output"))


def get_headers():
    """Return common headers for API requests."""
    return {
        "Content-Type": "application/json",
        "X-API-Key": API_KEY,
    }


def generate_invoice_data(invoice_num: str) -> dict:
    """Generate sample invoice data."""
    quantity = random.randint(1, 10)
    return {
        "invoice": {
            "number": f"INV-2025-{invoice_num}",
            "company": {
                "name": "Petty PDF Solutions Inc.",
                "address": "123 Document Lane",
                "city": "San Francisco",
                "zip": "94102"
            },
            "customer": {
                "name": f"Customer {invoice_num} Corp.",
                "email": f"billing{invoice_num}@example.com"
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
                    "quantity": quantity,
                    "price": "49.00",
                    "total": f"{49 * quantity}.00"
                }
            ],
            "subtotal": "594.00",
            "tax_rate": "8.5",
            "tax": "50.49",
            "total": "644.49"
        }
    }


def test_health() -> bool:
    """Test health endpoint."""
    print("[INFO] Testing health endpoint...")
    try:
        response = requests.get(f"{API_BASE}/health", timeout=10)
        if response.status_code == 200:
            print(f"[INFO] Health check passed: {response.json()}")
            return True
        else:
            print(f"[ERROR] Health check failed (HTTP {response.status_code}): {response.text}")
            return False
    except Exception as e:
        print(f"[ERROR] Health check failed: {e}")
        return False


def test_sync_generation(invoice_num: str = "001") -> bool:
    """Test synchronous PDF generation."""
    output_file = OUTPUT_DIR / f"sync-invoice-{invoice_num}.pdf"
    print(f"[INFO] Testing synchronous PDF generation (invoice #{invoice_num})...")

    request_body = {
        "template": "invoice",
        "data": generate_invoice_data(invoice_num)
    }

    try:
        response = requests.post(
            f"{API_BASE}/api/v1/generate",
            headers=get_headers(),
            json=request_body,
            timeout=60
        )

        if response.status_code == 200:
            OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
            output_file.write_bytes(response.content)
            file_size = len(response.content)
            print(f"[INFO] Sync generation succeeded: {output_file} ({file_size} bytes)")
            return True
        else:
            print(f"[ERROR] Sync generation failed (HTTP {response.status_code}): {response.text}")
            return False
    except Exception as e:
        print(f"[ERROR] Sync generation failed: {e}")
        return False


def test_async_generation(invoice_num: str = "001") -> bool:
    """Test asynchronous PDF generation."""
    print(f"[INFO] Testing asynchronous PDF generation (invoice #{invoice_num})...")

    request_body = {
        "template": "invoice",
        "data": generate_invoice_data(invoice_num)
    }

    try:
        # Create job
        response = requests.post(
            f"{API_BASE}/api/v1/jobs",
            headers=get_headers(),
            json=request_body,
            timeout=30
        )

        if response.status_code != 202:
            print(f"[ERROR] Job creation failed (HTTP {response.status_code}): {response.text}")
            return False

        job_data = response.json()
        job_id = job_data["job_id"]
        print(f"[INFO] Job created: {job_id}")

        # Poll for completion
        max_attempts = 30
        for attempt in range(1, max_attempts + 1):
            time.sleep(1)

            response = requests.get(
                f"{API_BASE}/api/v1/jobs/{job_id}",
                headers=get_headers(),
                timeout=10
            )

            if response.status_code != 200:
                print(f"[ERROR] Status check failed (HTTP {response.status_code}): {response.text}")
                return False

            status_data = response.json()
            status = status_data["status"]
            print(f"[INFO] Job status: {status} (attempt {attempt})")

            if status == "completed":
                break
            elif status == "failed":
                print(f"[ERROR] Job failed: {status_data.get('error', {}).get('message', 'Unknown error')}")
                return False

        if status != "completed":
            print("[ERROR] Job timed out")
            return False

        # Download result
        output_file = OUTPUT_DIR / f"async-invoice-{invoice_num}.pdf"
        response = requests.get(
            f"{API_BASE}/api/v1/jobs/{job_id}/download",
            headers=get_headers(),
            timeout=60
        )

        if response.status_code == 200:
            OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
            output_file.write_bytes(response.content)
            file_size = len(response.content)
            print(f"[INFO] Async generation succeeded: {output_file} ({file_size} bytes)")
            return True
        else:
            print(f"[ERROR] Download failed (HTTP {response.status_code}): {response.text}")
            return False

    except Exception as e:
        print(f"[ERROR] Async generation failed: {e}")
        return False


def test_load(count: int, parallel: int = 1, use_async: bool = False) -> None:
    """Run load test."""
    mode = "async" if use_async else "sync"
    print(f"[INFO] Starting load test: {count} invoices ({mode} mode, {parallel} concurrent)")

    start_time = time.time()
    success = 0
    failed = 0

    def generate_one(i: int) -> bool:
        invoice_num = f"{i:04d}"
        if use_async:
            return test_async_generation(invoice_num)
        else:
            return test_sync_generation(invoice_num)

    if parallel > 1:
        with ThreadPoolExecutor(max_workers=parallel) as executor:
            futures = {executor.submit(generate_one, i): i for i in range(1, count + 1)}
            for future in as_completed(futures):
                try:
                    if future.result():
                        success += 1
                    else:
                        failed += 1
                except Exception:
                    failed += 1
    else:
        for i in range(1, count + 1):
            try:
                if generate_one(i):
                    success += 1
                else:
                    failed += 1
            except Exception:
                failed += 1

    duration = time.time() - start_time
    rate = count / duration if duration > 0 else 0

    print()
    print("[INFO] Load test completed:")
    print(f"[INFO]   Total: {count}")
    print(f"[INFO]   Success: {success}")
    print(f"[INFO]   Failed: {failed}")
    print(f"[INFO]   Duration: {duration:.2f}s")
    print(f"[INFO]   Rate: {rate:.2f} PDFs/sec")
    if parallel > 1:
        print(f"[INFO]   Concurrency: {parallel}")


def run_all_tests() -> bool:
    """Run all tests."""
    print("[INFO] Running all API tests...")
    print()

    if not test_health():
        print("[ERROR] Health check failed. Is the service running?")
        return False

    print()
    if not test_sync_generation("sync-test"):
        print("[ERROR] Sync generation failed")
        return False

    print()
    if not test_async_generation("async-test"):
        print("[ERROR] Async generation failed")
        return False

    print()
    print("[INFO] All tests passed!")
    return True


def main():
    parser = argparse.ArgumentParser(description="PDF Service API Test Script")
    parser.add_argument("--health", action="store_true", help="Run health check only")
    parser.add_argument("--sync", action="store_true", help="Run sync generation test")
    parser.add_argument("--async", dest="async_test", action="store_true", help="Run async generation test")
    parser.add_argument("--load", type=int, metavar="COUNT", help="Run load test with COUNT invoices")
    parser.add_argument("--parallel", type=int, default=1, metavar="N", help="Number of concurrent requests for load test")
    parser.add_argument("--use-async", action="store_true", help="Use async mode for load test")

    args = parser.parse_args()

    # If no specific test requested, run all
    if not any([args.health, args.sync, args.async_test, args.load]):
        success = run_all_tests()
        sys.exit(0 if success else 1)

    if args.health:
        success = test_health()
        sys.exit(0 if success else 1)

    if args.sync:
        success = test_sync_generation()
        sys.exit(0 if success else 1)

    if args.async_test:
        success = test_async_generation()
        sys.exit(0 if success else 1)

    if args.load:
        test_load(args.load, args.parallel, args.use_async)
        sys.exit(0)


if __name__ == "__main__":
    main()
