fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = "proto";
    let protos = &[
        "proto/common.proto",
        "proto/auth.proto",
        "proto/calendar.proto",
        "proto/sync.proto",
        "proto/solver.proto",
        "proto/analytics.proto",
        "proto/mcp.proto",
    ];

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(protos, &[proto_dir])?;

    // Rerun if any proto changes
    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }

    Ok(())
}
