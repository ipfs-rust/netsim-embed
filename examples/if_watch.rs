use anyhow::Result;
use async_trait::async_trait;
use if_watch::{IfEvent, IfWatcher};
use ipnet::IpNet;
use netsim_embed_cli::{run_server, Server};

pub struct IfWatchServer;

#[async_trait]
impl Server for IfWatchServer {
    async fn start() -> Result<Self> {
        Ok(Self)
    }

    async fn run(&mut self) -> Result<()> {
        let mut watcher = IfWatcher::new().await?;
        loop {
            let watcher = &mut watcher;
            let event = watcher.await?;
            if let IfEvent::Up(IpNet::V4(ip)) = event {
                println!("got ip {}", ip);
                break;
            }
        }
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    run_server::<IfWatchServer>().await
}
