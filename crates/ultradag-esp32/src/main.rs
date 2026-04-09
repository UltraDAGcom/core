#![no_std]
#![no_main]

use esp32_hal::{
    prelude::*,
    timer::Timer,
    pac::Peripherals,
};
use panic_halt as _;

#[entry]
fn main() -> ! {
    let peripherals = Peripherals::take().unwrap();
    
    // Configure LED pin (GPIO2 is usually built-in LED)
    let mut led = peripherals.GPIO2.into_push_pull_output();
    
    // Configure timer for blinking
    let mut timer = Timer::new(peripherals.TIMG0);
    
    println!("UltraDAG ESP32 client starting...");
    
    let mut state = false;
    loop {
        // Toggle LED every second
        led.set_high().unwrap();
        timer.delay(1.secs());
        led.set_low().unwrap();
        timer.delay(1.secs());
        
        // Print status every 5 blinks
        if !state {
            println!("UltraDAG ESP32 client running...");
            println!("HTTP API would be available on port 80");
            println!("Endpoints: /status, /tx, /peers");
            state = true;
        }
    }
}
