use anyhow::Result;
use async_io::Async;
use async_trait::async_trait;
use futures::prelude::*;
use netsim_embed_cli::{run_server, Server};
use std::net::{SocketAddrV4, TcpListener};

pub struct TcpServer {
    listener: Async<TcpListener>,
}

#[async_trait]
impl Server for TcpServer {
    async fn start() -> Result<Self> {
        let addr = SocketAddrV4::new(0.into(), 3000);
        let listener = Async::<TcpListener>::bind(addr)?;
        Ok(Self { listener })
    }

    async fn run(&mut self) -> Result<()> {
        let incoming = self.listener.incoming();
        futures::pin_mut!(incoming);
        let mut stream = incoming.next().await.unwrap()?;
        let mut buf = [0u8; 11];
        let len = stream.read(&mut buf).await?;
        assert_eq!(&buf[..len], &b"ping"[..]);
        println!("received ping");
        stream.write_all(b"pong").await?;
        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    run_server::<TcpServer>().await
}
