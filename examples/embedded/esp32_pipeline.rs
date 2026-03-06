//! ESP32-S3 sensor pipeline example (conceptual — runs on any target)
//!
//! Demonstrates the canonical IoT pattern:
//!   sensor_read -> threshold_filter -> unit_transform -> packet_encode
//!
//! On a real ESP32, sensor_read would call esp-idf ADC APIs.
//! The DAG structure and execution are identical regardless of platform.
//!
//! Build for ESP32: cargo build --target xtensa-esp32s3-espidf --release
//! Build for Pi Zero: cargo build --target arm-unknown-linux-gnueabihf --release

use ultradag_core::{FnNode, GraphBuilder, TinyValue};
use ultradag_exec::SyncExecutor;

fn main() {
    let mut builder = GraphBuilder::new();

    // Stage 1: Read sensor (simulated ADC value)
    let read = builder
        .add_node(FnNode::new("adc_read", b"adc_v1".to_vec(), |_| {
            // On real hardware: esp_adc_read(channel) -> raw 12-bit value
            Ok(TinyValue::Int(2048)) // Midpoint of 12-bit ADC
        }))
        .unwrap();

    // Stage 2: Convert raw ADC to voltage
    let convert = builder
        .add_node(FnNode::new("adc_to_mv", b"adc_mv_v1".to_vec(), |inputs| {
            let raw = inputs[0].as_int().unwrap();
            let mv = (raw * 3300) / 4095; // 3.3V reference, 12-bit
            Ok(TinyValue::Int(mv))
        }))
        .unwrap();

    // Stage 3: Threshold filter (only pass if > 1000mV)
    let filter = builder
        .add_node(FnNode::new("threshold", b"thresh_v1".to_vec(), |inputs| {
            let mv = inputs[0].as_int().unwrap();
            if mv > 1000 {
                Ok(TinyValue::Int(mv))
            } else {
                Ok(TinyValue::Null)
            }
        }))
        .unwrap();

    // Stage 4: Pack into a transmission-ready byte payload
    let encode = builder
        .add_node(FnNode::new("encode", b"enc_v1".to_vec(), |inputs| {
            if inputs[0].is_null() {
                return Ok(TinyValue::Null);
            }
            let mv = inputs[0].as_int().unwrap();
            // Simple binary protocol: [0x01 (type), mv_high, mv_low]
            let payload = vec![0x01, (mv >> 8) as u8, (mv & 0xFF) as u8];
            Ok(TinyValue::Bytes(payload))
        }))
        .unwrap();

    builder.add_edge(read, convert).unwrap();
    builder.add_edge(convert, filter).unwrap();
    builder.add_edge(filter, encode).unwrap();

    let graph = builder.build().unwrap();

    // Execute — this is the same call on ESP32, Pi, or x86
    let result = SyncExecutor::new().execute(&graph).unwrap();

    let output = result.get(&encode).unwrap();
    match output {
        TinyValue::Bytes(payload) => {
            println!("TX payload: {:02X?}", payload);
            println!("Voltage: {}mV", ((payload[1] as i64) << 8) | payload[2] as i64);
        }
        TinyValue::Null => println!("Below threshold, no transmission"),
        _ => unreachable!(),
    }

    println!("Pipeline time: {:?}", result.trace.total_duration);
    println!("Nodes executed: {}", result.trace.node_count());
}
