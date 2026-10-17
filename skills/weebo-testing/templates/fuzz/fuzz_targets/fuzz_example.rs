// source: batlehub@20879e5 (fuzz/fuzz_targets/fuzz_path_safe.rs), reduced to its shape.
#![no_main]

use libfuzzer_sys::fuzz_target;

use my_crate::{my_parse, my_render};

fuzz_target!(|data: &[u8]| {
    // Structured input: decode what the parser takes; drop what cannot be decoded.
    let mut u = arbitrary::Unstructured::new(data);
    let Ok(input): arbitrary::Result<String> = u.arbitrary() else {
        return;
    };

    // "No panic" is the floor. Assert a property: here, a round trip.
    let Ok(parsed) = my_parse(&input) else {
        return;
    };
    let rendered = my_render(&parsed);
    assert_eq!(
        my_parse(&rendered).ok().as_ref(),
        Some(&parsed),
        "round trip changed the value: {input:?} -> {rendered:?}"
    );
});
