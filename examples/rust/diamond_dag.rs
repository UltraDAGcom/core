//! Diamond DAG with parallel branches
//!
//! Run with: cargo run --example diamond_dag

use ultradag_core::{FnNode, GraphBuilder, TinyValue};
use ultradag_exec::ParallelExecutor;

fn main() {
    let mut builder = GraphBuilder::new();

    //    source(100)
    //     /       \
    //  double    square
    //     \       /
    //      sum
    let source = builder
        .add_node(FnNode::new("source", b"src_v1".to_vec(), |_| {
            Ok(TinyValue::Int(100))
        }))
        .unwrap();

    let double = builder
        .add_node(FnNode::new("double", b"dbl_v1".to_vec(), |inputs| {
            let v = inputs[0].as_int().unwrap();
            Ok(TinyValue::Int(v * 2))
        }))
        .unwrap();

    let square = builder
        .add_node(FnNode::new("square", b"sq_v1".to_vec(), |inputs| {
            let v = inputs[0].as_int().unwrap();
            Ok(TinyValue::Int(v * v))
        }))
        .unwrap();

    let sum = builder
        .add_node(FnNode::new("sum", b"sum_v1".to_vec(), |inputs| {
            let total: i64 = inputs.iter().filter_map(|v| v.as_int()).sum();
            Ok(TinyValue::Int(total))
        }))
        .unwrap();

    builder.add_edge(source, double).unwrap();
    builder.add_edge(source, square).unwrap();
    builder.add_edge(double, sum).unwrap();
    builder.add_edge(square, sum).unwrap();

    let graph = builder.build().unwrap();

    // Execute in parallel — double and square run concurrently
    let result = ParallelExecutor::new().execute(&graph).unwrap();

    println!("source = {}", result.get(&source).unwrap().as_int().unwrap());
    println!("double = {}", result.get(&double).unwrap().as_int().unwrap());
    println!("square = {}", result.get(&square).unwrap().as_int().unwrap());
    println!("sum    = {}", result.get(&sum).unwrap().as_int().unwrap());
    // Expected: 200 + 10000 = 10200
    println!("Time: {:?}", result.trace.total_duration);
}
