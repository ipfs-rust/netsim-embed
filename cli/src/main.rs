use netsim_embed::{run, Ipv4Range, NetworkBuilder, Wire};
use netsim_embed_cli::{Command, Event};
use std::path::PathBuf;
use std::time::Duration;
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Opts {
    #[structopt(long)]
    topology: String,
    #[structopt(long)]
    client: PathBuf,
    #[structopt(long)]
    server: PathBuf,
    #[structopt(long)]
    delay_ms: Option<u64>,
}

fn main() {
    env_logger::init();
    run(async {
        let opts = Opts::from_args();
        let mut net = NetworkBuilder::new(Ipv4Range::global());
        let addr = net.random_client_addr();
        net.spawn_machine(
            Wire::new(),
            Some(addr),
            async_process::Command::new(opts.server),
        );
        let mut wire = Wire::new();
        if let Some(delay) = opts.delay_ms {
            wire.set_delay(Duration::from_millis(delay));
            wire.set_buffer_size(u64::MAX as usize);
        }
        let mut client = async_process::Command::new(opts.client);
        client.arg(addr.to_string());
        match opts.topology.as_str() {
            "m2" => {
                net.spawn_machine(wire, None, client).await;
            }
            "m1m1" => {
                let mut net2 = NetworkBuilder::new(Ipv4Range::global());
                net2.spawn_machine(wire, None, client).await;
                net.spawn_network(None, net2);
            }
            "m1nm1" => {
                let mut net2 = NetworkBuilder::new(Ipv4Range::global());
                net2.spawn_machine(wire, None, client).await;
                net.spawn_network(Some(Default::default()), net2);
            }
            _ => panic!("unsupported topology"),
        }
        let mut net = net.spawn();
        let server = net.machine(0);
        server.send(Command::Start).await;
        loop {
            if server.recv().await == Some(Event::Started) {
                break;
            }
        }
        let client = if net.machines().len() > 1 {
            net.machine(1)
        } else {
            net.subnet(0).machine(0)
        };
        client.send(Command::Start).await;
        loop {
            if client.recv().await == Some(Event::Started) {
                break;
            }
        }
        client.send(Command::Exit).await;
        loop {
            if client.recv().await == Some(Event::Exited) {
                break;
            }
        }
        let server = net.machine(0);
        server.send(Command::Exit).await;
        loop {
            if server.recv().await == Some(Event::Exited) {
                break;
            }
        }
    });
}
