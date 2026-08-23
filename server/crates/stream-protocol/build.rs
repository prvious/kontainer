fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc_path = protoc_bin_vendored::protoc_bin_path()?;

    // SAFETY: Cargo build scripts are single-threaded processes. The variable is
    // set before Prost spawns protoc and is not observed by application code.
    unsafe { std::env::set_var("PROTOC", protoc_path) };

    prost_build::Config::new()
        .compile_protos(&["proto/kontainer/stream/v1/stream.proto"], &["proto"])?;

    println!("cargo:rerun-if-changed=proto/kontainer/stream/v1/stream.proto");
    Ok(())
}
