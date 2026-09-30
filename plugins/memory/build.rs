#[path = "src/eval/fingerprint.rs"]
mod fingerprint;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let root = manifest.join("../..").canonicalize()?;
    for input in fingerprint::INPUTS {
        println!("cargo:rerun-if-changed={}", root.join(input).display());
    }
    println!(
        "cargo:rustc-env=ELEGY_MEMORY_BUILD_SOURCE_SHA256={}",
        fingerprint::source_digest(&root)?
    );
    Ok(())
}
