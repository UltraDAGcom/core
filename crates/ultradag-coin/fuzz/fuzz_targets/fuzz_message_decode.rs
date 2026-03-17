#![no_main]
use libfuzzer_sys::fuzz_target;

// Fuzz the postcard deserialization of DagVertex with signature verification.
// If deserialization succeeds, also try verify_signature() — it must not panic.
use ultradag_coin::DagVertex;

fuzz_target!(|data: &[u8]| {
    if let Ok(vertex) = postcard::from_bytes::<DagVertex>(data) {
        // These operations must never panic on arbitrary deserialized data:
        let _ = vertex.hash();
        let _ = vertex.signable_bytes();
        let _ = vertex.verify_signature();
        let _ = vertex.block.total_fees();
        let _ = vertex.block.compute_merkle_root();
    }
});
