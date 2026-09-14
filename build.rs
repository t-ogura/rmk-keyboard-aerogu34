//! Build script: the `memory.x` linker script, the Vial keyboard definition,
//! feature forwarding to rmk-macro, and a check that the BLE device name
//! fits.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::{env, fs};

use const_gen::*;
use xz2::read::XzEncoder;

/// trouble-host's GATT device name limit. RMK passes `product_name` straight
/// into `Server::new_with_config(...).unwrap()`, and trouble-host returns
/// `Err("Device name is too long. Max length is 22 bytes")` past this. The
/// unwrap panics on the first poll of the BLE transport, inside the same
/// `run_all` as the USB transport, so the board never finishes enumerating and
/// presents as completely dead -- no HID device, no serial port, no clue.
///
/// RMK does not check this at build time, so we do.
const DEVICE_NAME_MAX_LEN: usize = 22;

/// Vial caches a keyboard's definition against this id, so it must be unique
/// per keyboard *and* per layout: reusing an id with a different matrix makes
/// Vial see the device and then fail to open it. "AEROGU34".
const VIAL_KEYBOARD_ID: [u8; 8] = *b"AEROGU34";

fn keyboard_toml_path() -> PathBuf {
    println!("cargo:rerun-if-env-changed=KEYBOARD_TOML_PATH");
    PathBuf::from(env::var_os("KEYBOARD_TOML_PATH").unwrap_or_else(|| "keyboard.toml".into()))
}

fn check_device_name_length() {
    let path = keyboard_toml_path();
    let toml = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    for key in ["name", "product_name"] {
        for line in toml.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some(rest) = line.strip_prefix(key) else { continue };
            let Some(rest) = rest.trim_start().strip_prefix('=') else { continue };
            let value = rest.trim().trim_matches('"');
            if value.len() > DEVICE_NAME_MAX_LEN {
                panic!(
                    "keyboard.toml: `{key}` is {} bytes (\"{value}\"), but trouble-host \
                     rejects a GATT device name longer than {DEVICE_NAME_MAX_LEN}. RMK unwraps \
                     that error, so the firmware would panic before USB enumerates and the \
                     board would appear completely dead.",
                    value.len()
                );
            }
        }
    }
}

fn main() {
    check_device_name_length();

    println!("cargo:rerun-if-changed=vial.json");
    generate_vial_config();

    // Put `memory.x` in our output directory and ensure it's on the linker
    // search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=keyboard.toml");

    // Tell rmk-macro which rmk features this crate's own `vial` / `rynk`
    // features forward. The macro reads the feature list straight out of
    // Cargo.toml's `rmk` dependency and cannot see feature forwarding, so it
    // would otherwise reject the `[host]` block as inconsistent.
    let mut rmk_features: Vec<&str> = Vec::new();
    for (crate_feature, forwards) in [
        ("VIAL", &["vial", "host_lock"][..]),
        ("RYNK", &["rynk"][..]),
        ("DEFMT", &["defmt"][..]),
        ("USB_LOG", &["usb_log"][..]),
    ] {
        println!("cargo:rerun-if-env-changed=CARGO_FEATURE_{crate_feature}");
        if env::var_os(format!("CARGO_FEATURE_{crate_feature}")).is_some() {
            rmk_features.extend(forwards);
        }
    }
    println!("cargo:rustc-env=RMK_FEATURES={}", rmk_features.join(","));

    // `--nmagic` is required if memory section addresses are not aligned to
    // 0x10000, as FLASH and RAM in `memory.x` are not.
    println!("cargo:rustc-link-arg=--nmagic");
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-link-arg=-Tdefmt.x");
}

fn generate_vial_config() {
    let out_file = Path::new(&env::var_os("OUT_DIR").unwrap()).join("config_generated.rs");

    let p = Path::new("vial.json");
    let mut content = String::new();
    match File::open(p) {
        Ok(mut file) => {
            file.read_to_string(&mut content).expect("Cannot read vial.json");
        }
        Err(e) => println!("Cannot find vial.json {:?}: {}", p, e),
    };

    let vial_cfg = json::stringify(json::parse(&content).unwrap());
    let mut keyboard_def_compressed: Vec<u8> = Vec::new();
    XzEncoder::new(vial_cfg.as_bytes(), 6)
        .read_to_end(&mut keyboard_def_compressed)
        .unwrap();

    let keyboard_id: Vec<u8> = VIAL_KEYBOARD_ID.to_vec();
    let const_declarations = [
        const_declaration!(pub VIAL_KEYBOARD_DEF = keyboard_def_compressed),
        const_declaration!(pub VIAL_KEYBOARD_ID = keyboard_id),
    ]
    .map(|s| "#[allow(clippy::redundant_static_lifetimes)]\n".to_owned() + s.as_str())
    .join("\n");
    fs::write(out_file, const_declarations).unwrap();
}
