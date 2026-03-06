// TinyDAG JavaScript SDK Example
// Requires: npm install tinydag (after WASM build with wasm-pack)
//
// Build WASM: cd bindings/tinydag-wasm && wasm-pack build --target nodejs
// Then: node examples/javascript/basic.js

const { WasmGraphBuilder, WasmValue, blake3_hash } = require('../../bindings/tinydag-wasm/pkg');

// Hash some data
const hash = blake3_hash(Buffer.from('hello tinydag'));
console.log('Blake3 hash:', hash);

// Build a DAG
const builder = new WasmGraphBuilder();
const sensorId = builder.add_node('sensor', 'sensor_v1');
const filterId = builder.add_node('filter', 'filter_v1');
const outputId = builder.add_node('output', 'output_v1');

builder.add_edge(sensorId, filterId);
builder.add_edge(filterId, outputId);

console.log(`DAG: ${builder.node_count()} nodes, ${builder.edge_count()} edges`);

// Work with values
const val = WasmValue.int(42);
console.log(`Value: ${val.to_json()} (type: ${val.type_name()})`);
console.log(`As int: ${val.as_int()}`);

const nullVal = WasmValue.null();
console.log(`Null check: ${nullVal.is_null()}`);
