#![no_std]
#![no_main]

use esp_idf_hal::{
    prelude::*,
    task::block_on,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    http::server::{EspHttpServer, Configuration},
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, EspWifi},
};
use ultradag_esp32::{UltraDAGNode, NetworkConfig};

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
        ultradag_peers: vec![
            "ultradag-node-1.fly.dev:8080".to_string(),
            "ultradag-node-2.fly.dev:8080".to_string(),
        ],
    };

    // Initialize UltraDAG node
    let mut udag_node = UltraDAGNode::new(network_config);

    // Start HTTP server for API
    let mut server = EspHttpServer::new(&Configuration::default()).unwrap();
    
    server
        .handle_get("/status", move |req| {
            let status = udag_node.get_status();
            req.send_ok_response(
                Some(("application/json", status.as_bytes())),
            )
        })
        .unwrap();

    server
        .handle_post("/tx", move |mut req| {
            let mut buf = [0u8; 1024];
            let len = req.read(&mut buf).unwrap();
            let tx_data = &buf[..len];
            
            match udag_node.submit_transaction(tx_data) {
                Ok(hash) => req.send_ok_response(
                    Some(("application/json", format!("{{\"hash\":\"{}\"}}", hash).as_bytes())),
                ),
                Err(e) => req.send_response(
                    400,
                    Some(("application/json", format!("{{\"error\":\"{}\"}}", e).as_bytes())),
                ),
            }
        })
        .unwrap();

    println!("UltraDAG ESP32 node started!");
    println!("WiFi connecting...");
    
    // Connect to WiFi
    wifi.connect().unwrap();
    
    println!("WiFi connected!");
    println!("UltraDAG node ready on ESP32");

    loop {
        // Main loop - process network messages, consensus, etc.
        udag_node.tick();
        delay!(FreeRtos::tick_rate() * 1000); // 1 second delay
    }
}
