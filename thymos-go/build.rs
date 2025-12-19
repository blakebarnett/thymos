//! Build script for thymos-go
//!
//! This build script validates the build environment.
//! The C header (include/thymos.h) is manually maintained for
//! maximum CGO compatibility since cbindgen has limitations
//! with opaque Rust types.

fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=include/thymos.h");
}

