#![no_std]
#![no_main]

use esp_idf_hal::prelude::*;
use esp_idf_svc::{
    http::server::{EspHttpServer, Configuration},
    wifi::{BlockingWifi, EspWifi},
    nvs::EspDefaultNvsPartition,
    eventloop::EspSystemEventLoop,
};

#[entry]
fn main() -> ! {
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    let wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs)).unwrap(),
        sys_loop,
    );

    // Connect to WiFi
    println!("Connecting to WiFi...");
    match wifi.connect() {
        Ok(_) => println!("WiFi connected!"),
        Err(e) => println!("WiFi connection failed: {:?}", e),
    }

    // Start simple HTTP server
    let mut server = EspHttpServer::new(&Configuration::default()).unwrap();
    
    server
        .handle_get("/", move |req| {
            let response = r#"{
                "message": "UltraDAG ESP32 Client",
                "status": "running",
                "network": "ultradag"
            }"#;
            req.send_ok_response(Some(("application/json", response.as_bytes())))
        })
        .unwrap();

    println!("UltraDAG ESP32 client running!");
    println!("Visit http://<ESP32_IP>/ to check status");

    loop {
        // Simple delay
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
