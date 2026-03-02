fn main() {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(
            &["proto/wal.proto"],
            &["proto"],
        )
        .unwrap();
}
