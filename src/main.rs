#![forbid(unsafe_code)]

// Phase 0 deliberately defines no command-line interface. This zero-interface
// entry point exists only so release builds can prove the standalone ELF target.
fn main() {}
