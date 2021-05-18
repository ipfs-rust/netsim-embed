use anyhow::Result;
use async_trait::async_trait;
use netsim_embed_cli::{run_server, Server};
use std::io::IoSliceMut;
use std::net::SocketAddrV4;
use udp_socket::{RecvMeta, UdpSocket};

pub struct UdpServer {
    socket: UdpSocket,
}

#[async_trait]
impl Server for UdpServer {
    async fn start() -> Result<Self> {
        let addr = SocketAddrV4::new(0.into(), 3000);
        let socket = UdpSocket::bind(addr.into())?;
        Ok(Self { socket })
    }

    async fn run(&mut self) -> Result<()> {
        let mut meta = [RecvMeta::default()];
        let mut data = [0; 16];
        let mut buffer = [IoSliceMut::new(&mut data[..])];
        for _ in 0..3 {
            println!("receiving");
            self.socket.recv(&mut buffer, &mut meta).await?;
            let slice = &buffer[0][..meta[0].len];
            println!("received {:?}", slice);
        }
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    run_server::<UdpServer>().await
}
