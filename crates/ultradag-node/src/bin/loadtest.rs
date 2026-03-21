use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{info, warn};

use ultradag_coin::SecretKey;

#[derive(Clone)]
struct LoadTestConfig {
    node_url: String,
    num_transactions: usize,
    concurrent_requests: usize,
    sender_secret: String,
    receiver_address: String,
}

#[derive(Debug)]
struct LoadTestError(String);

impl std::fmt::Display for LoadTestError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LoadTestError {}

impl From<reqwest::Error> for LoadTestError {
    fn from(e: reqwest::Error) -> Self {
        LoadTestError(e.to_string())
    }
}

impl From<serde_json::Error> for LoadTestError {
    fn from(e: serde_json::Error) -> Self {
        LoadTestError(e.to_string())
    }
}

async fn send_transaction(
    client: &reqwest::Client,
    config: &LoadTestConfig,
    _nonce: u64,
) -> Result<(Duration, String), LoadTestError> {
    let start = Instant::now();
    
    let body = serde_json::json!({
        "from_secret": config.sender_secret,
        "to": config.receiver_address,
        "amount": 1000,
        "fee": 100,
    });

    let response = client
        .post(format!("{}/tx", config.node_url))
        .json(&body)
        .send()
        .await?;

    let elapsed = start.elapsed();
    let status = response.status();
    let text = response.text().await.map_err(|e| LoadTestError(e.to_string()))?;

    if !status.is_success() {
        return Err(LoadTestError(format!("HTTP {}: {}", status, text)));
    }

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| LoadTestError(e.to_string()))?;
    let tx_hash = json["hash"].as_str().unwrap_or("unknown").to_string();

    Ok((elapsed, tx_hash))
}

async fn get_balance(
    client: &reqwest::Client,
    node_url: &str,
    address: &str,
) -> Result<u64, LoadTestError> {
    let response = client
        .get(format!("{}/balance/{}", node_url, address))
        .send()
        .await
        .map_err(|e| LoadTestError(e.to_string()))?;

    let json: serde_json::Value = response.json().await
        .map_err(|e| LoadTestError(e.to_string()))?;
    Ok(json["balance"].as_u64().unwrap_or(0))
}

async fn get_status(
    client: &reqwest::Client,
    node_url: &str,
) -> Result<serde_json::Value, LoadTestError> {
    let response = client.get(format!("{}/status", node_url)).send().await
        .map_err(|e| LoadTestError(e.to_string()))?;
    response.json().await.map_err(|e| LoadTestError(e.to_string()))
}

#[tokio::main]
async fn main() -> Result<(), LoadTestError> {
    tracing_subscriber::fmt::init();

    // Generate test accounts
    let sender_sk = SecretKey::generate();
    let sender_addr = sender_sk.address();
    let receiver_sk = SecretKey::generate();
    let receiver_addr = receiver_sk.address();

    info!("Load Test Configuration:");
    info!("Sender: {}", sender_addr.to_hex());
    info!("Receiver: {}", receiver_addr.to_hex());
    info!("Sender secret: {}", hex::encode(sender_sk.to_bytes()));

    let config = LoadTestConfig {
        node_url: "http://127.0.0.1:8001".to_string(),
        num_transactions: 1000,
        concurrent_requests: 50,
        sender_secret: hex::encode(sender_sk.to_bytes()),
        receiver_address: receiver_addr.to_hex(),
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    info!("\n=== Phase 1: Fund sender account ===");
    
    // Get initial balance
    let initial_balance = get_balance(&client, &config.node_url, &sender_addr.to_hex()).await?;
    info!("Initial sender balance: {} sats", initial_balance);

    if initial_balance == 0 {
        warn!("Sender has no balance. You need to:");
        warn!("1. Start a validator node that will mine blocks");
        warn!("2. Wait for some blocks to be mined");
        warn!("3. Send coins to the sender address: {}", sender_addr.to_hex());
        warn!("\nOr use the /keygen endpoint to get a funded validator address");
        return Ok(());
    }

    // Calculate max transactions we can send
    let tx_cost = 1000 + 100; // amount + fee
    let max_txs = initial_balance / tx_cost;
    let num_txs = std::cmp::min(config.num_transactions, max_txs as usize);

    info!("Can send {} transactions with current balance", max_txs);
    info!("Will send {} transactions", num_txs);

    info!("\n=== Phase 2: Load test - sending {} transactions ===", num_txs);
    
    let start_time = Instant::now();
    let semaphore = Arc::new(Semaphore::new(config.concurrent_requests));
    let mut tasks = Vec::new();

    for i in 0..num_txs {
        let client = client.clone();
        let config = config.clone();
        let semaphore = semaphore.clone();

        let task = tokio::spawn(async move {
            let _permit = match semaphore.acquire().await {
                Ok(permit) => permit,
                Err(_) => return Err(LoadTestError("semaphore closed".into())),
            };
            send_transaction(&client, &config, i as u64).await
        });

        tasks.push(task);
    }

    let mut successful = 0;
    let mut failed = 0;
    let mut total_latency = Duration::ZERO;
    let mut min_latency = Duration::MAX;
    let mut max_latency = Duration::ZERO;

    for (i, task) in tasks.into_iter().enumerate() {
        match task.await {
            Ok(Ok((latency, _hash))) => {
                successful += 1;
                total_latency += latency;
                min_latency = min_latency.min(latency);
                max_latency = max_latency.max(latency);

                if (i + 1) % 100 == 0 {
                    info!("Progress: {}/{} transactions sent", i + 1, num_txs);
                }
            }
            Ok(Err(e)) => {
                failed += 1;
                if failed <= 5 {
                    warn!("Transaction failed: {}", e);
                }
            }
            Err(e) => {
                failed += 1;
                warn!("Task failed: {}", e);
            }
        }
    }

    let total_duration = start_time.elapsed();

    info!("\n=== Phase 3: Results ===");
    info!("Total time: {:.2}s", total_duration.as_secs_f64());
    info!("Successful: {}", successful);
    info!("Failed: {}", failed);
    info!("Success rate: {:.1}%", (successful as f64 / num_txs as f64) * 100.0);
    
    if successful > 0 {
        let avg_latency = total_latency / successful as u32;
        info!("\nLatency:");
        info!("  Min: {:.2}ms", min_latency.as_secs_f64() * 1000.0);
        info!("  Avg: {:.2}ms", avg_latency.as_secs_f64() * 1000.0);
        info!("  Max: {:.2}ms", max_latency.as_secs_f64() * 1000.0);
        
        let tps = successful as f64 / total_duration.as_secs_f64();
        info!("\nThroughput: {:.2} TPS (transactions per second)", tps);
    }

    // Wait a bit for transactions to be processed
    info!("\n=== Phase 4: Waiting for finalization (10 seconds) ===");
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Check final status
    let status = get_status(&client, &config.node_url).await?;
    info!("\nFinal network status:");
    info!("{}", serde_json::to_string_pretty(&status)?);

    let final_balance = get_balance(&client, &config.node_url, &sender_addr.to_hex()).await?;
    let receiver_balance = get_balance(&client, &config.node_url, &config.receiver_address).await?;
    
    info!("\nFinal balances:");
    info!("  Sender: {} sats (spent {} sats)", final_balance, initial_balance - final_balance);
    info!("  Receiver: {} sats", receiver_balance);

    Ok(())
}
