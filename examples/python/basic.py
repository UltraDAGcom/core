"""
UltraDAG Python SDK Example

Build: cd bindings/ultradag-python && maturin develop
Then:  python examples/python/basic.py
"""

import ultradag

# Hash some data
h = ultradag.blake3_hash(b"hello ultradag")
print(f"Blake3 hash: {h}")

# Build a DAG
builder = ultradag.PyGraphBuilder()
sensor_id = builder.add_node("sensor", "sensor_v1")
filter_id = builder.add_node("filter", "filter_v1")

builder.add_edge(sensor_id, filter_id)

print(f"DAG: {builder.node_count()} nodes, {builder.edge_count()} edges")
print(f"Sensor ID: {sensor_id[:16]}...")
print(f"Filter ID: {filter_id[:16]}...")
