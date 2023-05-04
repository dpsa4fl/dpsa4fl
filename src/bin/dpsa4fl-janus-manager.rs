#[tokio::main]
async fn main() -> anyhow::Result<()>
{
    dpsa4fl::janus_manager::interface::network::provider::main().await
}
