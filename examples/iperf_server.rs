use anyhow::{Context, Result};
use async_process::{Child, Command};
use async_trait::async_trait;
use netsim_embed_cli::{run_server, Server};

pub struct IperfServer {
    child: Child,
}

#[async_trait]
impl Server for IperfServer {
    async fn start() -> Result<Self> {
        let child = Command::new("iperf")
            .arg("-s")
            .arg("-w")
            .arg("1M")
            .arg("-m")
            .spawn()
            .with_context(|| "running iperf")?;
        Ok(Self { child })
    }

    async fn exit(&mut self) -> Result<()> {
        self.child.kill()?;
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    run_server::<IperfServer>().await
}
