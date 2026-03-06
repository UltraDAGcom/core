//! IoT sensor pipeline: read -> filter -> transform -> transmit
//!
//! Run with: cargo run --example sensor_pipeline

use ultradag_core::{FnNode, GraphBuilder, TinyValue};
use ultradag_exec::SyncExecutor;
use ultradag_runtime::ExecutionProof;

fn main() {
    // Build the DAG
    let mut builder = GraphBuilder::new();

    let read = builder
        .add_node(FnNode::new("sensor_read", b"sensor_v1".to_vec(), |_| {
            Ok(TinyValue::Float(23.5)) // Simulated temperature reading
        }))
        .unwrap();

    let filter = builder
        .add_node(FnNode::new(
            "range_filter",
            b"filter_v1".to_vec(),
            |inputs| {
                let temp = inputs[0].as_float().unwrap();
                if (10.0..50.0).contains(&temp) {
                    Ok(inputs[0].clone())
                } else {
                    Ok(TinyValue::Null)
                }
            },
        ))
        .unwrap();

    let transform = builder
        .add_node(FnNode::new(
            "celsius_to_fahrenheit",
            b"c2f_v1".to_vec(),
            |inputs| {
                let c = inputs[0].as_float().unwrap();
                Ok(TinyValue::Float(c * 9.0 / 5.0 + 32.0))
            },
        ))
        .unwrap();

    let transmit = builder
        .add_node(FnNode::new("transmit", b"tx_v1".to_vec(), |inputs| {
            let f = inputs[0].as_float().unwrap();
            let json = format!("{{\"temperature_f\":{f:.1},\"unit\":\"F\"}}");
            Ok(TinyValue::Bytes(json.into_bytes()))
        }))
        .unwrap();

    builder.add_edge(read, filter).unwrap();
    builder.add_edge(filter, transform).unwrap();
    builder.add_edge(transform, transmit).unwrap();

    let graph = builder.build().unwrap();
    println!(
        "DAG built: {} nodes, {} edges",
        graph.node_count(),
        graph.edge_count()
    );

    // Execute
    let result = SyncExecutor::new().execute(&graph).unwrap();

    // Show results
    let payload = result.get(&transmit).unwrap();
    let bytes = payload.as_bytes().unwrap();
    println!("Output: {}", std::str::from_utf8(bytes).unwrap());
    println!("Execution time: {:?}", result.trace.total_duration);

    // Generate proof
    let graph_hash = *blake3::hash(b"sensor_pipeline_v1").as_bytes();
    let proof =
        ExecutionProof::from_execution(graph_hash, &result, graph.execution_order());
    println!("Proof verified: {}", proof.verify());
    println!(
        "Merkle root: {}",
        proof
            .merkle_root
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
}
