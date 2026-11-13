use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    metrics_api::serve(SocketAddr::from(([127, 0, 0, 1], 8080))).await
}
