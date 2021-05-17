use anyhow::Result;
use async_io::Async;
use async_trait::async_trait;
use netsim_embed_cli::{run_server, Server};
use std::net::{SocketAddrV4, UdpSocket};

pub struct UdpServer {
    socket: Async<UdpSocket>,
}

#[async_trait]
impl Server for UdpServer {
    async fn start() -> Result<Self> {
        let addr = SocketAddrV4::new(0.into(), 3000);
        let socket = async_io::Async::<UdpSocket>::bind(addr)?;
        Ok(Self { socket })
    }

    async fn run(&mut self) -> Result<()> {
        loop {
            let mut buf = [0u8; 11];
            let (len, addr) = self.socket.recv_from(&mut buf).await.unwrap();
            if &buf[..len] == b"ping" {
                println!("received ping");

                self.socket.send_to(b"pong", addr).await.unwrap();
                break;
            }
        }
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    run_server::<UdpServer>().await
}
