//! Build script for dynamic platform-specific optimizations
//!
//! This script detects the target platform and applies appropriate
//! optimizations for Thymos and its dependencies.

use std::env;

fn main() {
    let target = env::var("TARGET").unwrap();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=TARGET");

    // Print build information for debugging
    println!("cargo:warning=Building for target: {}", target);
    println!(
        "cargo:warning=Target OS: {}, Architecture: {}",
        target_os, target_arch
    );

    // Configure platform-specific optimizations
    configure_platform_optimizations(&target_os, &target_arch);

    // Use LLD linker if available for faster builds
    configure_linker(&target_os);
}

fn configure_platform_optimizations(target_os: &str, target_arch: &str) {
    match target_os {
        "linux" => {
            // Linux-specific optimizations
            match target_arch {
                "x86_64" => {
                    println!("cargo:rustc-cfg=feature=\"optimized_x86_64\"");
                }
                "aarch64" => {
                    println!("cargo:rustc-cfg=feature=\"optimized_aarch64\"");
                }
                _ => {}
            }
        }

        "macos" => {
            // macOS-specific optimizations
            println!("cargo:rustc-cfg=feature=\"macos_optimized\"");

            if target_arch == "aarch64" {
                println!("cargo:rustc-cfg=feature=\"apple_silicon\"");
            }
        }

        "windows" => {
            // Windows-specific optimizations
            println!("cargo:rustc-cfg=feature=\"windows_optimized\"");
        }

        _ => {}
    }
}

fn configure_linker(target_os: &str) {
    if target_os == "linux" {
        // Use LLD linker for faster linking on Linux if available
        if which::which("ld.lld").is_ok() {
            println!("cargo:rustc-link-arg=-fuse-ld=lld");
            println!("cargo:warning=Using LLD linker for faster builds");
        } else {
            println!("cargo:warning=LLD not available, using default linker");
        }
    }
}
