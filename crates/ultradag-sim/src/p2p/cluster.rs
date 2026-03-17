use std::process::{Child, Command, Stdio};
use std::time::Duration;
use crate::p2p::helpers;

/// A running node process.
pub struct TestNode {
    pub port: u16,
    pub rpc_port: u16,
    pub index: usize,
    process: Child,
    #[allow(dead_code)]
    data_dir: tempfile::TempDir, // Kept alive to prevent temp dir cleanup while node runs
}

impl Drop for TestNode {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// A cluster of real node processes on localhost.
pub struct TestCluster {
    pub nodes: Vec<TestNode>,
    pub base_port: u16,
}

impl TestCluster {
    /// Spawn `num_nodes` real node processes on localhost.
    /// Each connects to all others via --seed flags.
    pub fn new(num_nodes: usize, base_port: u16) -> Self {
        let binary = helpers::node_binary();
        let mut nodes = Vec::new();

        for i in 0..num_nodes {
            let port = base_port + i as u16;
            let rpc_port = port + 1000;
            let data_dir = tempfile::TempDir::new().expect("Failed to create temp dir");

            let mut cmd = Command::new(&binary);
            cmd.arg("--port").arg(port.to_string())
               .arg("--rpc-port").arg(rpc_port.to_string())
               .arg("--validators").arg(num_nodes.to_string())
               .arg("--no-bootstrap")
               .arg("--data-dir").arg(data_dir.path().to_str().unwrap())
               .arg("--testnet")
               .arg("--validate")
               .arg("--round-ms").arg("2000")
               .arg("--skip-fast-sync"); // No fast-sync for local tests

            // Add seed peers (all other nodes)
            for j in 0..num_nodes {
                if j != i {
                    let seed_port = base_port + j as u16;
                    cmd.arg("--seed").arg(format!("127.0.0.1:{}", seed_port));
                }
            }

            cmd.stdout(Stdio::null())
               .stderr(Stdio::inherit()); // Show node logs for debugging

            eprintln!("[TestCluster] Spawning node {} on port {} (binary: {})", i, port, binary);
            let process = cmd.spawn()
                .unwrap_or_else(|e| panic!("Failed to spawn node {}: {}. Binary: {}", i, e, binary));

            nodes.push(TestNode { port, rpc_port, index: i, process, data_dir });
        }

        Self { nodes, base_port }
    }

    /// Wait for all nodes to be reachable via RPC (raw TCP health check).
    pub async fn wait_for_ready(&self, timeout_secs: u64) -> bool {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

        loop {
            if tokio::time::Instant::now() > deadline { return false; }

            let mut all_ready = true;
            for node in &self.nodes {
                let addr = format!("127.0.0.1:{}", node.rpc_port);
                match tokio::time::timeout(
                    Duration::from_secs(2),
                    tokio::net::TcpStream::connect(&addr),
                ).await {
                    Ok(Ok(_)) => {} // Port open
                    _ => { all_ready = false; break; }
                }
            }
            if all_ready { return true; }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Wait for all nodes to finalize past `min_round`.
    pub async fn wait_for_finality(&self, min_round: u64, timeout_secs: u64) -> bool {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

        loop {
            if tokio::time::Instant::now() > deadline { return false; }

            let mut all_past = true;
            for node in &self.nodes {
                match self.http_get_json(node.rpc_port, "/status").await {
                    Some(json) => {
                        let round = json["last_finalized_round"].as_u64().unwrap_or(0);
                        if round < min_round { all_past = false; break; }
                    }
                    None => { all_past = false; break; }
                }
            }
            if all_past { return true; }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    /// Get status JSON from a node.
    pub async fn get_status(&self, node_idx: usize) -> Option<serde_json::Value> {
        self.http_get_json(self.nodes[node_idx].rpc_port, "/status").await
    }

    /// Raw HTTP GET returning parsed JSON. No reqwest dependency.
    async fn http_get_json(&self, port: u16, path: &str) -> Option<serde_json::Value> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let addr = format!("127.0.0.1:{}", port);
        let mut stream = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::TcpStream::connect(&addr),
        ).await.ok()?.ok()?;

        let request = format!("GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n", path, port);
        stream.write_all(request.as_bytes()).await.ok()?;

        let mut response = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(5),
            stream.read_to_end(&mut response),
        ).await.ok()?.ok()?;

        let response_str = String::from_utf8_lossy(&response);
        // Find the JSON body after the blank line
        let body_start = response_str.find("\r\n\r\n").map(|i| i + 4)?;
        let body = &response_str[body_start..];
        serde_json::from_str(body).ok()
    }
}

impl Drop for TestCluster {
    fn drop(&mut self) {
        for node in &mut self.nodes {
            let _ = node.process.kill();
        }
    }
}
