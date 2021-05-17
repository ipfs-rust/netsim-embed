use anyhow::Result;
use async_trait::async_trait;
use netsim_embed_cli::{run_client, Client};
use std::net::{Ipv4Addr, SocketAddrV4};
use udp_socket::{Transmit, UdpSocket};

pub struct UdpClient;

#[async_trait]
impl Client for UdpClient {
    async fn run(&mut self, addr: Ipv4Addr) -> Result<()> {
        let bind_addr = SocketAddrV4::new(0.into(), 3000);
        let socket = UdpSocket::bind(bind_addr.into())?;
        let transmit = Transmit {
            destination: SocketAddrV4::new(addr, 3000).into(),
            contents: [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2].to_vec(),
            segment_size: Some(4),
            ecn: None,
            src_ip: None,
        };
        socket.send(&[transmit]).await?;
        println!("sent");
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    run_client(UdpClient).await
}
