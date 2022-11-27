use anyhow::{Context, Result};
use async_process::Command;
use async_trait::async_trait;
use netsim_embed_cli::{run_client, Client};
use std::net::Ipv4Addr;

pub struct IperfClient;

#[async_trait]
impl Client for IperfClient {
    async fn run(&mut self, addr: Ipv4Addr) -> Result<()> {
        Command::new("iperf")
            .arg("-c")
            .arg(format!("{}", addr))
            .arg("-w")
            .arg("1M")
            .arg("-m")
            .spawn()
            .with_context(|| "running iperf")?
            .status()
            .await
            .unwrap();
        Command::new("netstat")
            .arg("-s")
            .spawn()
            .unwrap()
            .status()
            .await
            .unwrap();
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    run_client(IperfClient).await
}
