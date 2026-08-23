fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc_path = protoc_bin_vendored::protoc_bin_path()?;

    // SAFETY: Cargo build scripts are single-threaded processes. The variable is
    // set before prost spawns protoc and is not observed by application code.
    unsafe { std::env::set_var("PROTOC", protoc_path) };

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/kontainer/agent/v1/agent.proto"], &["proto"])?;

    println!("cargo:rerun-if-changed=proto/kontainer/agent/v1/agent.proto");
    Ok(())
}
