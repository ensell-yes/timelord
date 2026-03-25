// Lazy tonic client for optional auth introspection (non-hot-path).
// Hot-path validation uses local RS256 JWT verification in middleware/auth.rs.
#![allow(dead_code)]

use timelord_proto::timelord::auth::auth_service_client::AuthServiceClient;
use tonic::transport::Channel;

pub async fn connect(url: &str) -> anyhow::Result<AuthServiceClient<Channel>> {
    let channel = Channel::from_shared(url.to_string())?.connect().await?;
    Ok(AuthServiceClient::new(channel))
}
