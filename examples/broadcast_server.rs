use anyhow::Result;
use async_io::Async;
use async_trait::async_trait;
use netsim_embed_cli::{run_server, Server};
use std::net::{Ipv4Addr, UdpSocket};

pub struct BroadcastServer {
    socket: Async<UdpSocket>,
}

#[async_trait]
impl Server for BroadcastServer {
    async fn start() -> Result<Self> {
        let socket = async_io::Async::<UdpSocket>::bind((Ipv4Addr::UNSPECIFIED, 5353))?;
        let multicast = [224, 0, 0, 251].into();
        socket
            .get_ref()
            .join_multicast_v4(&multicast, &Ipv4Addr::UNSPECIFIED)
            .unwrap();
        Ok(Self { socket })
    }

    async fn run(&mut self) -> Result<()> {
        loop {
            let mut buf = [0u8; 11];
            let (len, _addr) = self.socket.recv_from(&mut buf).await?;
            if &buf[..len] == b"broadcast" {
                println!("received broadcast message");
                break;
            }
        }
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    run_server::<BroadcastServer>().await
}
