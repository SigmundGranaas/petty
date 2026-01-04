# PDF-as-a-Service Example

A production-ready web service demonstrating how to use Petty PDF Generator in a RESTful API. Supports both **synchronous** (immediate response) and **asynchronous** (job-based) PDF generation.

## Features

- **Synchronous Generation**: POST template + data → receive PDF immediately
- **Asynchronous Jobs**: POST job → receive job ID → poll for completion → download PDF
- **Template Caching**: Pre-compiled XSLT templates for high performance
- **Bounded Concurrency**: Semaphore-based limits prevent resource exhaustion
- **PostgreSQL Job Queue**: Reliable, persistent job storage with ACID guarantees
- **Filesystem Storage**: Generated PDFs stored on disk (extensible to S3/Azure)
- **API Key Authentication**: Simple security via `X-API-Key` header
- **Structured Logging**: JSON-formatted logs with tracing
- **Health Checks**: `/health` endpoint for monitoring

## Architecture

```
┌─────────────┐
│   Client    │
└──────┬──────┘
       │
       ├─ POST /api/v1/generate (sync)
       │  └─> PDF bytes (immediate)
       │
       ├─ POST /api/v1/jobs (async)
       │  └─> Job ID
       │
       └─ GET /api/v1/jobs/{id}
          └─> Job status + download URL

┌──────────────────────────────────────┐
│         PDF Service (Axum)           │
├──────────────────────────────────────┤
│ ┌──────────────┐  ┌───────────────┐ │
│ │ API Handlers │  │ Middleware    │ │
│ │ - Sync       │  │ - Auth        │ │
│ │ - Async      │  │ - Tracing     │ │
│ └──────┬───────┘  └───────────────┘ │
│        │                             │
│ ┌──────┴──────────────────────────┐ │
│ │   PipelineManager (Cached)     │ │
│ │   ┌──────┐  ┌──────┐           │ │
│ │   │ XSLT │  │ JSON │  ...      │ │
│ │   └──────┘  └──────┘           │ │
│ └────────────────┬────────────────┘ │
│                  │                   │
│ ┌────────────────┴────────────────┐ │
│ │    Petty DocumentPipeline       │ │
│ │    (Parallel PDF Rendering)     │ │
│ └─────────────────────────────────┘ │
└──────────────────────────────────────┘
         │                  │
    ┌────┴──────┐    ┌─────┴──────┐
    │ PostgreSQL │    │ Background │
    │ Job Queue  │    │  Workers   │
    └────────────┘    └────────────┘
                            │
                      ┌─────┴──────┐
                      │ Filesystem │
                      │  Storage   │
                      └────────────┘
```

## Prerequisites

- Rust 1.75+
- PostgreSQL 14+
- System fonts (for text rendering)

## Quick Start

```bash
# 1. Start PostgreSQL (using Docker)
cd examples/pdf_service
docker compose up -d

# 2. Run the service (from any directory)
cargo run -p pdf-service

# 3. Run the Rust test client
cargo run -p pdf-service --bin test-client
```

The service can be run from either the workspace root or the `examples/pdf_service/` directory - config paths are resolved automatically.

## Setup

### 1. Database Setup

**Option A: Docker Compose (recommended)**

```bash
cd examples/pdf_service
docker compose up -d
```

**Option B: Manual PostgreSQL**

```bash
# Create database
createdb pdf_service

# Set DATABASE_URL
export DATABASE_URL="postgresql://postgres:password@localhost:5432/pdf_service"
```

### 2. Configuration

Copy and edit the environment variables:

```bash
cp examples/pdf_service/.env.example examples/pdf_service/.env
```

Edit `.env`:

```env
DATABASE_URL=postgresql://postgres:password@localhost:5432/pdf_service
SERVER_HOST=0.0.0.0
SERVER_PORT=3000
TEMPLATE_DIR=./examples/pdf_service/templates
STORAGE_PATH=./examples/pdf_service/output
API_KEY=your-secret-api-key-here
MAX_CONCURRENT_SYNC=10
WORKER_COUNT=4
RUST_LOG=info,pdf_service=debug
```

### 3. Run the Service

```bash
# Build and run
cargo run --bin pdf-service

# Or with release optimizations
cargo run --release --bin pdf-service
```

The service will:
1. Run database migrations
2. Load and compile XSLT templates from `templates/`
3. Start 4 background workers (configurable)
4. Listen on `http://0.0.0.0:3000`

## API Reference

### Authentication

All API endpoints (except `/health`) require an API key via the `X-API-Key` header:

```bash
curl -H "X-API-Key: your-secret-api-key-here" ...
```

### Endpoints

#### 1. Health Check

```bash
GET /health
```

**Response:**
```json
{
  "status": "ok",
  "service": "pdf-service",
  "version": "0.1.0"
}
```

#### 2. Synchronous Generation

Generate a PDF immediately and receive bytes in the response.

```bash
POST /api/v1/generate
Content-Type: application/json
X-API-Key: your-secret-api-key-here

{
  "template": "invoice",
  "data": {
    "invoice": {
      "number": "INV-2025-001",
      "company": {
        "name": "Acme Corp",
        "address": "123 Main St",
        "city": "San Francisco",
        "zip": "94102"
      },
      "customer": {
        "name": "John Doe",
        "email": "john@example.com"
      },
      "items": [
        {
          "description": "Consulting Services",
          "quantity": 10,
          "price": "150.00",
          "total": "1500.00"
        }
      ],
      "subtotal": "1500.00",
      "tax_rate": "8.5",
      "tax": "127.50",
      "total": "1627.50"
    }
  }
}
```

**Response:**
- Status: `200 OK`
- Content-Type: `application/pdf`
- Body: PDF bytes

**Example:**

```bash
curl -X POST http://localhost:3000/api/v1/generate \
  -H "Content-Type: application/json" \
  -H "X-API-Key: dev-secret-key" \
  -d @examples/pdf_service/templates/sample_invoice_data.json \
  --output invoice.pdf
```

#### 3. Async Job Creation

Create a PDF generation job and receive a job ID.

```bash
POST /api/v1/jobs
Content-Type: application/json
X-API-Key: your-secret-api-key-here

{
  "template": "invoice",
  "data": { /* same as sync */ }
}
```

**Response:**
```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "pending",
  "created_at": "2025-12-21T10:30:00Z",
  "status_url": "/api/v1/jobs/550e8400-e29b-41d4-a716-446655440000"
}
```

**Example:**

```bash
curl -X POST http://localhost:3000/api/v1/jobs \
  -H "Content-Type: application/json" \
  -H "X-API-Key: dev-secret-key" \
  -d @examples/pdf_service/templates/sample_invoice_data.json
```

#### 4. Job Status

Check the status of an async job.

```bash
GET /api/v1/jobs/{job_id}
X-API-Key: your-secret-api-key-here
```

**Response (Pending):**
```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "pending",
  "created_at": "2025-12-21T10:30:00Z",
  "updated_at": "2025-12-21T10:30:00Z"
}
```

**Response (Processing):**
```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "processing",
  "created_at": "2025-12-21T10:30:00Z",
  "updated_at": "2025-12-21T10:30:05Z",
  "started_at": "2025-12-21T10:30:05Z"
}
```

**Response (Completed):**
```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "completed",
  "created_at": "2025-12-21T10:30:00Z",
  "updated_at": "2025-12-21T10:30:12Z",
  "completed_at": "2025-12-21T10:30:12Z",
  "result": {
    "download_url": "/api/v1/jobs/550e8400-e29b-41d4-a716-446655440000/download",
    "file_size": 245678,
    "expires_at": "2025-12-22T10:30:12Z"
  }
}
```

**Response (Failed):**
```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "failed",
  "created_at": "2025-12-21T10:30:00Z",
  "updated_at": "2025-12-21T10:30:08Z",
  "error": {
    "message": "Template 'unknown' not found"
  }
}
```

**Example:**

```bash
curl http://localhost:3000/api/v1/jobs/550e8400-e29b-41d4-a716-446655440000 \
  -H "X-API-Key: dev-secret-key"
```

#### 5. Download Job Result

Download the generated PDF for a completed job.

```bash
GET /api/v1/jobs/{job_id}/download
X-API-Key: your-secret-api-key-here
```

**Response:**
- Status: `200 OK`
- Content-Type: `application/pdf`
- Body: PDF bytes

**Example:**

```bash
curl http://localhost:3000/api/v1/jobs/550e8400-e29b-41d4-a716-446655440000/download \
  -H "X-API-Key: dev-secret-key" \
  --output result.pdf
```

## Adding Templates

1. Create an XSLT file in `examples/pdf_service/templates/`:

```bash
examples/pdf_service/templates/
├── invoice.xsl          # Sample invoice template
└── my_template.xsl      # Your new template
```

2. Restart the service (templates are loaded at startup)

3. Use the template name (filename without extension) in API requests:

```json
{
  "template": "my_template",
  "data": { /* your data */ }
}
```

## Configuration

### Concurrency Tuning

**Synchronous Endpoint:**
- `MAX_CONCURRENT_SYNC`: Limits concurrent sync PDF generations (default: 10)
- Higher = more throughput, but more memory usage
- Tune based on available RAM and CPU cores

**Async Workers:**
- `WORKER_COUNT`: Number of background workers processing jobs (default: 4)
- Higher = faster job processing, but more memory usage
- Recommended: 1-2 workers per CPU core

**Petty Pipeline:**
- `worker_threads` (config.toml): Petty internal parallelism (default: 4)
- `render_buffer_size` (config.toml): Buffering for render commands (default: 32)

### Storage

Currently uses filesystem storage. To extend to S3/Azure:

1. Implement `Storage` trait in `src/storage/s3.rs`
2. Update `main.rs` to select backend based on `STORAGE_BACKEND` env var

## Performance

### Throughput Benchmarks

On 8-core machine with simple invoice template:

- **Sync endpoint**: ~150-200 PDFs/sec (limited by semaphore)
- **Async workers**: ~500-1000 PDFs/sec (4 workers, batched)
- **Memory**: ~500MB total (4 workers, bounded queue)

### Optimization Tips

1. **Increase worker count** for async jobs: `WORKER_COUNT=8`
2. **Increase sync semaphore** for sync endpoint: `MAX_CONCURRENT_SYNC=20`
3. **Use Petty parallelism**: `worker_threads=8` in config.toml
4. **Enable release mode**: `cargo run --release`
5. **Use connection pooling**: Already configured for PostgreSQL

## Error Handling

All errors return JSON responses:

```json
{
  "error": "ErrorCode",
  "message": "Human-readable message"
}
```

**Common Error Codes:**
- `TemplateNotFound`: Template name doesn't exist
- `GenerationFailed`: PDF rendering error
- `ServiceOverloaded`: Too many concurrent requests (503)
- `JobNotFound`: Invalid job ID (404)
- `Unauthorized`: Missing or invalid API key (401)
- `InvalidRequest`: Malformed request (400)

## Testing

### Test Client (Rust)

A Rust CLI test client is provided for testing the API:

```bash
# Run all tests (health, sync, async)
cargo run -p pdf-service --bin test-client

# Health check only
cargo run -p pdf-service --bin test-client -- health

# Sync generation test
cargo run -p pdf-service --bin test-client -- sync

# Async job test
cargo run -p pdf-service --bin test-client -- async

# Load test (10 PDFs sequentially)
cargo run -p pdf-service --bin test-client -- load --count 10

# Load test with concurrency
cargo run -p pdf-service --bin test-client -- load --count 50 --parallel 5

# Load test with async mode
cargo run -p pdf-service --bin test-client -- load --count 50 --parallel 5 --use-async
```

**Environment Variables:**

```bash
API_BASE=http://localhost:3000    # Service URL
API_KEY=dev-secret-key            # API key
OUTPUT_DIR=./test-output          # Where to save PDFs
```

### Shell Script (Alternative)

A bash test script is also available:

```bash
cd examples/pdf_service

# Run all tests
./test-api.sh

# Specific tests
./test-api.sh health
./test-api.sh sync
./test-api.sh async
./test-api.sh load 10
./test-api.sh parallel 50 5
```

## Development

### Run with hot-reload (cargo-watch)

```bash
cargo install cargo-watch
cargo watch -x 'run --bin pdf-service'
```

### Reload templates without restart

```bash
# Add an endpoint in api/mod.rs:
POST /api/v1/admin/reload-templates

# In handler:
state.pipeline_manager.reload().await?;
```

### Database Queries

```sql
-- View all jobs
SELECT id, template, status, created_at FROM jobs ORDER BY created_at DESC LIMIT 10;

-- Count by status
SELECT status, COUNT(*) FROM jobs GROUP BY status;

-- Cleanup old completed jobs
DELETE FROM jobs WHERE status = 'completed' AND completed_at < NOW() - INTERVAL '7 days';
```

## Production Deployment

### Docker

```dockerfile
FROM rust:1.78 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin pdf-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 fonts-liberation
COPY --from=builder /app/target/release/pdf-service /usr/local/bin/
COPY examples/pdf_service/templates /app/templates
COPY examples/pdf_service/config /app/config
WORKDIR /app
EXPOSE 3000
CMD ["pdf-service"]
```

### Environment Variables

```env
DATABASE_URL=postgresql://prod_user:secure_password@db.example.com:5432/pdf_service
API_KEY=production-secret-key-generate-with-openssl-rand
STORAGE_BACKEND=s3
STORAGE_PATH=s3://my-bucket/pdfs
RUST_LOG=info
```

### Monitoring

- **Logs**: Structured JSON logs (compatible with ELK, Datadog, etc.)
- **Health Check**: `GET /health` for load balancer probes
- **Metrics**: Extend with Prometheus metrics (see commented code)

## License

Same as Petty project (see root LICENSE file).
