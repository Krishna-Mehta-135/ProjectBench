use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    metrics_api::serve(SocketAddr::from(([0, 0, 0, 0], 8080))).await
}
