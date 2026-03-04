fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Slint コンパイル
    slint_build::compile("ui/app-window.slint")?;

    // Proto コード生成 (クライアントのみ)
    let protos = &[
        "zaciraci/crates/web/proto/zaciraci/v1/health.proto",
        "zaciraci/crates/web/proto/zaciraci/v1/config.proto",
        "zaciraci/crates/web/proto/zaciraci/v1/portfolio.proto",
    ];
    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(protos, &["zaciraci/crates/web/proto"])?;
    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }
    Ok(())
}
