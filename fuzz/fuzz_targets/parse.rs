//! Fuzz target: the markdown → `RuleSet` parser must never panic.
//!
//! Every malformed input must produce a typed error (or parse), never a
//! panic. Any panic found here is a bug in `agentforge-domain`.
//!
//! Run with a nightly toolchain:
//!
//! ```sh
//! cargo +nightly install cargo-fuzz
//! cargo +nightly fuzz run parse
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let _ = agentforge_domain::parse_agents_md(source, "fuzz");
    let _ = agentforge_domain::validate_agents_md(source);
});