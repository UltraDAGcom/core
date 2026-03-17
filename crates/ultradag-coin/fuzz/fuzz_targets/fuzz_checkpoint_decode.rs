#![no_main]
use libfuzzer_sys::fuzz_target;
use ultradag_coin::Checkpoint;

fuzz_target!(|data: &[u8]| {
    let _ = postcard::from_bytes::<Checkpoint>(data);
});
