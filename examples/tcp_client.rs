use anyhow::Result;
use async_trait::async_trait;
use futures::prelude::*;
use netsim_embed_cli::{run_client, Client};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};

pub struct TcpClient;

#[async_trait]
impl Client for TcpClient {
    async fn run(&mut self, addr: Ipv4Addr) -> Result<()> {
        let addr = SocketAddrV4::new(addr, 3000);
        let mut stream = async_io::Async::<TcpStream>::connect(addr).await?;
        stream.write_all(b"ping").await?;
        let mut buf = [0u8; 11];
        let len = stream.read(&mut buf).await?;
        assert_eq!(&buf[..len], &b"pong"[..]);
        println!("received pong");
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    run_client(TcpClient).await
}
