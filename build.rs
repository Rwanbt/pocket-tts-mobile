//! Embeds the Kyutai model configs (`vendor/kyutai-configs/*.yaml`) so the binary needs
//! neither a Python install nor a copy of the Python package.
//!
//! Adapted from <https://github.com/kyutai-labs/pocket-tts> (`rust/build.rs`), MIT.
//! The upstream script also embeds the web page of the Python package, which the mobile
//! engine has no use for.

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let configs_dir = manifest_dir.join("vendor").join("kyutai-configs");

    println!("cargo:rerun-if-changed={}", configs_dir.display());

    let mut configs: Vec<(String, PathBuf)> = std::fs::read_dir(&configs_dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", configs_dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "yaml"))
        .map(|path| (path.file_stem().unwrap().to_string_lossy().into_owned(), path))
        .collect();
    configs.sort();

    let mut out = String::from("/// The configs of `pocket_tts/config`, by language name.\n");
    out.push_str("pub const CONFIGS: &[(&str, &str)] = &[\n");
    for (name, path) in configs.iter() {
        println!("cargo:rerun-if-changed={}", path.display());
        writeln!(out, "    ({name:?}, include_str!({:?})),", path).unwrap();
    }
    out.push_str("];\n");

    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("embedded.rs");
    std::fs::write(out_path, out).unwrap();
}
