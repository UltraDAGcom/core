//! Runtime configuration baked into the firmware.
//!
//! This file is checked in with placeholder credentials. Before flashing,
//! edit the constants below to match your WiFi network, your UltraDAG
//! node's RPC URL, and the account address you want to watch. Nothing
//! is read from flash NVS — everything lives in .rodata.
//!
//! See `README.md` for a walkthrough.

/// WiFi SSID — must be 2.4 GHz. ESP32 (classic) does not do 5 GHz.
/// Emoji SSIDs are valid UTF-8 and fit in the 32-byte ESP-IDF SSID buffer
/// if your AP name contains one.
pub const WIFI_SSID: &str = "YOUR_WIFI_SSID";

/// WiFi password. WPA2-PSK only. Open networks won't work without code
/// changes (esp-idf-svc defaults to WPA2-Personal).
pub const WIFI_PASSWORD: &str = "YOUR_WIFI_PASSWORD";

/// URL of an UltraDAG node's HTTP RPC endpoint.
///
/// **Easiest (dev loop):** run an UltraDAG node on your laptop and point
/// at its LAN IP, e.g. `http://192.168.1.42:10333`. No TLS needed. This
/// assumes your ESP32 and laptop are on the same WiFi.
///
/// **Production-ish:** a public HTTPS URL like
/// `https://ultradag-node-1.fly.dev` (port 443 is implicit). This path
/// requires the mbedTLS root-cert bundle (already enabled in
/// `sdkconfig.defaults`) and adds ~60 KB of flash.
///
/// No trailing slash. Using the public fly.io testnet node 1 over HTTPS —
/// verified reachable (round ~14,898, 5 validators). Fly terminates TLS
/// at the edge and forwards to the node's internal port 10333.
pub const NODE_URL: &str = "https://ultradag-node-1.fly.dev";

/// Account addresses to watch — hex-encoded 20-byte addresses (no `0x`
/// prefix and no `@name` lookups). Each tick the client polls
/// `GET /balance/{addr}` for every entry and logs the result. Cap at 8 to
/// keep the per-tick latency reasonable — each address is one extra
/// HTTPS round trip (~3 s on this board).
///
/// The first entry is also used as the demo wallet's transfer recipient
/// in the once-per-boot wallet flow, so keep at least one entry.
pub const WATCH_ADDRESSES: &[&str] = &[
    "0000000000000000000000000000000000000001",
    // Add more addresses here to monitor multiple accounts simultaneously.
    // "abcdef…",
];

/// How often to poll the node, in seconds. 10 s is a sensible dev default —
/// aggressive enough to see new rounds land, gentle enough not to hammer
/// a testnet node or drain a laptop battery.
pub const POLL_INTERVAL_SECS: u64 = 10;
