//! PDF Service API Test Client
//!
//! A Rust CLI tool for testing the PDF Service API.
//!
//! # Usage
//!
//! ```bash
//! # Run all tests
//! cargo run -p pdf-service --bin test-client
//!
//! # Health check only
//! cargo run -p pdf-service --bin test-client -- health
//!
//! # Sync generation test
//! cargo run -p pdf-service --bin test-client -- sync
//!
//! # Async job test
//! cargo run -p pdf-service --bin test-client -- async
//!
//! # Load test (50 PDFs, 5 concurrent)
//! cargo run -p pdf-service --bin test-client -- load --count 50 --parallel 5
//! ```

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "test-client")]
#[command(about = "PDF Service API Test Client")]
struct Cli {
    /// API base URL
    #[arg(long, default_value = "http://localhost:3000", env = "API_BASE")]
    api_base: String,

    /// API key for authentication
    #[arg(long, default_value = "dev-secret-key", env = "API_KEY")]
    api_key: String,

    /// Output directory for generated PDFs
    #[arg(long, default_value = "./test-output", env = "OUTPUT_DIR")]
    output_dir: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run health check
    Health,

    /// Test synchronous PDF generation
    Sync {
        /// Invoice number suffix
        #[arg(default_value = "001")]
        invoice_num: String,
    },

    /// Test asynchronous PDF generation
    Async {
        /// Invoice number suffix
        #[arg(default_value = "001")]
        invoice_num: String,
    },

    /// Run load test
    Load {
        /// Number of PDFs to generate
        #[arg(short, long, default_value = "10")]
        count: usize,

        /// Number of concurrent requests
        #[arg(short, long, default_value = "1")]
        parallel: usize,

        /// Use async mode instead of sync
        #[arg(long)]
        use_async: bool,
    },
}

#[derive(Debug, Serialize)]
struct GenerateRequest {
    template: String,
    data: Value,
}

#[derive(Debug, Deserialize)]
struct JobCreateResponse {
    job_id: String,
    #[allow(dead_code)]
    status: String,
}

#[derive(Debug, Deserialize)]
struct JobStatusResponse {
    #[allow(dead_code)]
    job_id: String,
    status: String,
    #[serde(default)]
    error: Option<ErrorInfo>,
}

#[derive(Debug, Deserialize)]
struct ErrorInfo {
    message: String,
}

fn generate_invoice_data(invoice_num: &str) -> Value {
    let mut rng = rand::thread_rng();
    let quantity: u32 = rng.gen_range(1..=10);

    json!({
        "customers": {
            "item": {
                "invoiceNumber": format!("INV-2025-{}", invoice_num),
                "name": format!("Customer {} Corp.", invoice_num),
                "address": format!("123 Document Lane, San Francisco, CA 94102"),
                "items": {
                    "item": [
                        {
                            "product": "PDF Generation Service - Premium Plan",
                            "quantity": "1",
                            "price": "$299.00"
                        },
                        {
                            "product": format!("API Calls ({} x 10,000 requests)", quantity),
                            "quantity": quantity.to_string(),
                            "price": "$49.00"
                        },
                        {
                            "product": "Priority Support",
                            "quantity": "1",
                            "price": "$99.00"
                        }
                    ]
                }
            }
        }
    })
}

struct TestClient {
    client: Client,
    api_base: String,
    api_key: String,
    output_dir: PathBuf,
}

impl TestClient {
    fn new(api_base: String, api_key: String, output_dir: PathBuf) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_base,
            api_key,
            output_dir,
        }
    }

    async fn health_check(&self) -> anyhow::Result<bool> {
        println!("[INFO] Testing health endpoint...");

        let response = self
            .client
            .get(format!("{}/health", self.api_base))
            .send()
            .await?;

        if response.status().is_success() {
            let body: Value = response.json().await?;
            println!("[INFO] Health check passed: {}", body);
            Ok(true)
        } else {
            println!(
                "[ERROR] Health check failed (HTTP {}): {}",
                response.status(),
                response.text().await?
            );
            Ok(false)
        }
    }

    async fn sync_generate(&self, invoice_num: &str) -> anyhow::Result<bool> {
        let output_file = self
            .output_dir
            .join(format!("sync-invoice-{}.pdf", invoice_num));

        println!(
            "[INFO] Testing synchronous PDF generation (invoice #{})...",
            invoice_num
        );

        let request = GenerateRequest {
            template: "invoice".to_string(),
            data: generate_invoice_data(invoice_num),
        };

        let response = self
            .client
            .post(format!("{}/api/v1/generate", self.api_base))
            .header("Content-Type", "application/json")
            .header("X-API-Key", &self.api_key)
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            fs::create_dir_all(&self.output_dir)?;
            let bytes = response.bytes().await?;
            fs::write(&output_file, &bytes)?;
            println!(
                "[INFO] Sync generation succeeded: {} ({} bytes)",
                output_file.display(),
                bytes.len()
            );
            Ok(true)
        } else {
            println!(
                "[ERROR] Sync generation failed (HTTP {}): {}",
                response.status(),
                response.text().await?
            );
            Ok(false)
        }
    }

    async fn async_generate(&self, invoice_num: &str) -> anyhow::Result<bool> {
        println!(
            "[INFO] Testing asynchronous PDF generation (invoice #{})...",
            invoice_num
        );

        let request = GenerateRequest {
            template: "invoice".to_string(),
            data: generate_invoice_data(invoice_num),
        };

        // Create job
        let response = self
            .client
            .post(format!("{}/api/v1/jobs", self.api_base))
            .header("Content-Type", "application/json")
            .header("X-API-Key", &self.api_key)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            println!(
                "[ERROR] Job creation failed (HTTP {}): {}",
                response.status(),
                response.text().await?
            );
            return Ok(false);
        }

        let job: JobCreateResponse = response.json().await?;
        println!("[INFO] Job created: {}", job.job_id);

        // Poll for completion
        let max_attempts = 30;
        for attempt in 1..=max_attempts {
            tokio::time::sleep(Duration::from_secs(1)).await;

            let response = self
                .client
                .get(format!("{}/api/v1/jobs/{}", self.api_base, job.job_id))
                .header("X-API-Key", &self.api_key)
                .send()
                .await?;

            if !response.status().is_success() {
                println!(
                    "[ERROR] Status check failed (HTTP {}): {}",
                    response.status(),
                    response.text().await?
                );
                return Ok(false);
            }

            let status: JobStatusResponse = response.json().await?;
            println!("[INFO] Job status: {} (attempt {})", status.status, attempt);

            match status.status.as_str() {
                "completed" => break,
                "failed" => {
                    let msg = status
                        .error
                        .map(|e| e.message)
                        .unwrap_or_else(|| "Unknown error".to_string());
                    println!("[ERROR] Job failed: {}", msg);
                    return Ok(false);
                }
                _ => continue,
            }
        }

        // Download result
        let output_file = self
            .output_dir
            .join(format!("async-invoice-{}.pdf", invoice_num));

        let response = self
            .client
            .get(format!(
                "{}/api/v1/jobs/{}/download",
                self.api_base, job.job_id
            ))
            .header("X-API-Key", &self.api_key)
            .send()
            .await?;

        if response.status().is_success() {
            fs::create_dir_all(&self.output_dir)?;
            let bytes = response.bytes().await?;
            fs::write(&output_file, &bytes)?;
            println!(
                "[INFO] Async generation succeeded: {} ({} bytes)",
                output_file.display(),
                bytes.len()
            );
            Ok(true)
        } else {
            println!(
                "[ERROR] Download failed (HTTP {}): {}",
                response.status(),
                response.text().await?
            );
            Ok(false)
        }
    }

    async fn load_test(
        &self,
        count: usize,
        parallel: usize,
        use_async: bool,
    ) -> anyhow::Result<()> {
        let mode = if use_async { "async" } else { "sync" };
        println!(
            "[INFO] Starting load test: {} invoices ({} mode, {} concurrent)",
            count, mode, parallel
        );

        let start = Instant::now();
        let success = Arc::new(AtomicUsize::new(0));
        let failed = Arc::new(AtomicUsize::new(0));

        let pb = ProgressBar::new(count as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
                .unwrap()
                .progress_chars("##-"),
        );

        let semaphore = Arc::new(tokio::sync::Semaphore::new(parallel));
        let mut handles = Vec::new();

        for i in 1..=count {
            let permit = semaphore.clone().acquire_owned().await?;
            let client = self.client.clone();
            let api_base = self.api_base.clone();
            let api_key = self.api_key.clone();
            let output_dir = self.output_dir.clone();
            let success = success.clone();
            let failed = failed.clone();
            let pb = pb.clone();

            let handle = tokio::spawn(async move {
                let invoice_num = format!("{:04}", i);
                let test_client = TestClient {
                    client,
                    api_base,
                    api_key,
                    output_dir,
                };

                let result = if use_async {
                    test_client.async_generate(&invoice_num).await
                } else {
                    test_client.sync_generate(&invoice_num).await
                };

                match result {
                    Ok(true) => success.fetch_add(1, Ordering::SeqCst),
                    _ => failed.fetch_add(1, Ordering::SeqCst),
                };

                pb.inc(1);
                drop(permit);
            });

            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }

        pb.finish_with_message("done");

        let duration = start.elapsed();
        let success_count = success.load(Ordering::SeqCst);
        let failed_count = failed.load(Ordering::SeqCst);
        let rate = count as f64 / duration.as_secs_f64();

        println!();
        println!("[INFO] Load test completed:");
        println!("[INFO]   Total: {}", count);
        println!("[INFO]   Success: {}", success_count);
        println!("[INFO]   Failed: {}", failed_count);
        println!("[INFO]   Duration: {:.2?}", duration);
        println!("[INFO]   Rate: {:.2} PDFs/sec", rate);
        println!("[INFO]   Concurrency: {}", parallel);

        Ok(())
    }

    async fn run_all_tests(&self) -> anyhow::Result<bool> {
        println!("[INFO] Running all API tests...\n");

        if !self.health_check().await? {
            println!("[ERROR] Health check failed. Is the service running?");
            return Ok(false);
        }
        println!();

        if !self.sync_generate("sync-test").await? {
            println!("[ERROR] Sync generation failed");
            return Ok(false);
        }
        println!();

        if !self.async_generate("async-test").await? {
            println!("[ERROR] Async generation failed");
            return Ok(false);
        }
        println!();

        println!("[INFO] All tests passed!");
        Ok(true)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let client = TestClient::new(cli.api_base, cli.api_key, cli.output_dir);

    match cli.command {
        None => {
            let success = client.run_all_tests().await?;
            std::process::exit(if success { 0 } else { 1 });
        }
        Some(Commands::Health) => {
            let success = client.health_check().await?;
            std::process::exit(if success { 0 } else { 1 });
        }
        Some(Commands::Sync { invoice_num }) => {
            let success = client.sync_generate(&invoice_num).await?;
            std::process::exit(if success { 0 } else { 1 });
        }
        Some(Commands::Async { invoice_num }) => {
            let success = client.async_generate(&invoice_num).await?;
            std::process::exit(if success { 0 } else { 1 });
        }
        Some(Commands::Load {
            count,
            parallel,
            use_async,
        }) => {
            client.load_test(count, parallel, use_async).await?;
        }
    }

    Ok(())
}
