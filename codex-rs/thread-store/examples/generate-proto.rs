use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = PathBuf::from(std::env::args().nth(1).expect("proto dir"));
    let proto_file = proto_dir.join("codex.thread_store.v1.proto");

    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .out_dir(&proto_dir)
        .compile_protos(&[proto_file], &[proto_dir])?;

    Ok(())
}
