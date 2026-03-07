/// Public bootstrap nodes for the UltraDAG testnet.
/// A new node with no --seed flag will attempt to connect to these automatically.
/// These are dedicated IPv4 addresses on Fly.io with TCP port 9333 exposed.
pub const TESTNET_BOOTSTRAP_NODES: &[&str] = &[
    "206.51.242.223:9333",  // ultradag-node-1 (ams)
    "137.66.57.226:9333",   // ultradag-node-2 (ams)
    "169.155.54.169:9333",  // ultradag-node-3 (ams)
    "169.155.55.151:9333",  // ultradag-node-4 (ams)
];
