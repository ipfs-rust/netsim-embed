use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::net::Ipv4Addr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Start,
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Start => write!(f, ">start")?,
        }
        Ok(())
    }
}

impl std::str::FromStr for Command {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            ">start" => Self::Start,
            _ => return Err(anyhow!("invalid command")),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    Started,
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Started => write!(f, "<started")?,
        }
        Ok(())
    }
}

impl std::str::FromStr for Event {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "<started" => Self::Started,
            _ => return Err(anyhow!("invalid event")),
        })
    }
}

#[async_trait]
pub trait Server: Send + Sized {
    async fn start() -> Result<Self>;
    async fn run(&mut self) -> Result<()> {
        Ok(())
    }
    async fn exit(&mut self) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
pub trait Client {
    async fn run(&mut self, addr: Ipv4Addr) -> Result<()>;
}

pub async fn run_server<S: Server>() -> Result<()> {
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let mut server = S::start().await?;
    println!("{}", Event::Started);

    server.run().await?;

    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    server.exit().await?;
    Ok(())
}

pub async fn run_client<C: Client>(mut client: C) -> Result<()> {
    let addr: Ipv4Addr = std::env::args().nth(1).unwrap().parse()?;

    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    println!("{}", Event::Started);

    client.run(addr).await?;

    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(())
}
