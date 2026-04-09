#![no_std]
#![no_main]

use esp_idf_hal::{
    prelude::*,
    task::block_on,
    delay::{FreeRtos, Delay},
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    http::server::{EspHttpServer, Configuration},
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, EspWifi},
};
use ultradag_esp32::{UltraDAGClient, NetworkConfig, Transaction};
use heapless::{String, Vec};
use embedded_io::Write;

#[entry]
fn main() -> ! {
    esp_idf_svc::log::EspLogger::initialize_default();

    // Initialize peripherals
    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    // Initialize WiFi
    let wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs)).unwrap(),
        sys_loop,
    );

    // Configure network
    let network_config = NetworkConfig {
        ssid: "YOUR_WIFI_SSID",
        password: "YOUR_WIFI_PASSWORD",
        ultradag_peers: Vec::new(), // Start empty, will discover peers
    };

    // Initialize UltraDAG client
    let mut udag_client = UltraDAGClient::new(network_config);

    // Start HTTP server for API
    let mut server = EspHttpServer::new(&Configuration::default()).unwrap();
    
    // Status endpoint
    server
        .handle_get("/status", move |req| {
            let status = udag_client.get_status();
            req.send_ok_response(
                Some(("application/json", status.as_bytes())),
            )
        })
        .unwrap();

    // Transaction submission endpoint
    server
        .handle_post("/tx", move |mut req| {
            let mut buf = [0u8; 512];
            let len = req.read(&mut buf).unwrap();
            let tx_data = &buf[..len];
            
            // Simple transaction format: from:to:amount
            let tx_str = core::str::from_utf8(tx_data).unwrap_or("");
            let parts: Vec<&str, 3> = tx_str.split(':').collect();
            
            if parts.len() != 3 {
                return req.send_response(
                    400,
                    Some(("application/json", b"{\"error\":\"Invalid format. Use: from:to:amount\"}")),
                );
            }
            
            // Parse addresses (simplified)
            let from_addr = [0u8; 20]; // TODO: Parse hex address
            let to_addr = [0u8; 20];  // TODO: Parse hex address
            let amount: u64 = parts[2].parse().unwrap_or(0);
            
            let tx = udag_client.create_simple_tx(from_addr, to_addr, amount);
            
            match udag_client.submit_transaction(tx) {
                Ok(hash) => {
                    let response = format!("{{\"hash\":\"{}\",\"status\":\"pending\"}}", hash);
                    req.send_ok_response(
                        Some(("application/json", response.as_bytes())),
                    )
                },
                Err(e) => req.send_response(
                    400,
                    Some(("application/json", format!("{{\"error\":\"{}\"}}", e).as_bytes())),
                ),
            }
        })
        .unwrap();

    // Peers endpoint
    server
        .handle_get("/peers", move |req| {
            let peers_info = format!(
                r#"{{"connected_peers":{},"known_peers":{},"network":"ultradag"}}"#,
                1, // Simplified
                0
            );
            req.send_ok_response(
                Some(("application/json", peers_info.as_bytes())),
            )
        })
        .unwrap();

    println!("UltraDAG ESP32 client started!");
    println!("WiFi connecting...");
    
    // Connect to WiFi
    match wifi.connect() {
        Ok(_) => println!("WiFi connected!"),
        Err(e) => println!("WiFi connection failed: {:?}", e),
    }
    
    println!("UltraDAG client ready on ESP32");
    println!("Available endpoints:");
    println!("  GET  /status - Get client status");
    println!("  POST /tx     - Submit transaction (format: from:to:amount)");
    println!("  GET  /peers  - Get peer information");

    let mut delay = Delay::new_default();
    
    loop {
        // Main loop - maintain connection and process pending transactions
        udag_client.tick();
        
        // Blink LED to show activity
        // TODO: Add LED blink
        
        // 1 second delay
        delay.delay_ms(1000);
    }
}
