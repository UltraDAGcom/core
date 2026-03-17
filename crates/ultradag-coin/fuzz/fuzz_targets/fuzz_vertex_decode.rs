#![no_main]
use libfuzzer_sys::fuzz_target;
use ultradag_coin::DagVertex;

fuzz_target!(|data: &[u8]| {
    // Try to deserialize random bytes as a DagVertex.
    // Must never panic — only return Ok or Err.
    let _ = postcard::from_bytes::<DagVertex>(data);
});
