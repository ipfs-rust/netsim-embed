use anyhow::Result;
use async_trait::async_trait;
use netsim_embed_cli::{run_client, Client};
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

pub struct BroadcastClient;

#[async_trait]
impl Client for BroadcastClient {
    async fn run(&mut self, _addr: Ipv4Addr) -> Result<()> {
        let socket = async_io::Async::<UdpSocket>::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        let multicast = [224, 0, 0, 251].into();
        socket
            .send_to(b"broadcast", SocketAddrV4::new(multicast, 5353))
            .await?;

        println!("sent broadcast message");
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    run_client(BroadcastClient).await
}
