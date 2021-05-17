use anyhow::Result;
use async_trait::async_trait;
use netsim_embed_cli::{run_client, Client};
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

pub struct UdpClient;

#[async_trait]
impl Client for UdpClient {
    async fn run(&mut self, addr: Ipv4Addr) -> Result<()> {
        let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
        let socket = async_io::Async::<UdpSocket>::bind(bind_addr)?;
        socket
            .send_to(b"ping", SocketAddrV4::new(addr, 3000))
            .await?;

        let mut buf = [0u8; 11];
        let (len, _addr) = socket.recv_from(&mut buf).await?;
        assert_eq!(&buf[..len], b"pong");
        println!("received pong");
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    run_client(UdpClient).await
}
