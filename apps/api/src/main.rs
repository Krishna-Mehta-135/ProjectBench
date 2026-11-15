use std::net::SocketAddr;

fn bind_address(address: &str, operator_token: Option<&str>) -> anyhow::Result<SocketAddr> {
    let address: SocketAddr = address.parse()?;
    if !address.ip().is_loopback() && operator_token.is_none_or(|token| token.trim().is_empty()) {
        anyhow::bail!("METRICS_OPERATOR_TOKEN is required for a non-loopback API address");
    }
    Ok(address)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let configured =
        std::env::var("METRICS_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let token = std::env::var("METRICS_OPERATOR_TOKEN").ok();
    metrics_api::serve(bind_address(&configured, token.as_deref())?).await
}

#[cfg(test)]
mod tests {
    use super::bind_address;

    #[test]
    fn bind_requires_token_outside_loopback() {
        assert!(bind_address("127.0.0.1:8080", None).is_ok());
        assert!(bind_address("0.0.0.0:8080", None).is_err());
        assert!(bind_address("0.0.0.0:8080", Some(" ")).is_err());
        assert!(bind_address("0.0.0.0:8080", Some("secret")).is_ok());
    }
}
