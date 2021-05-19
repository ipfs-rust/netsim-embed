use netsim_embed::{run, DelayBuffer, Ipv4Range, NatConfig, Netsim};
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
        let mut sim = Netsim::new();
        let public = sim.spawn_network(Ipv4Range::global().split(2)[0]);
        let server_addr = sim.network(public).random_addr();
        let delay = opts.delay_ms.map(|delay| {
            let mut buffer = DelayBuffer::new();
            buffer.set_delay(Duration::from_millis(delay));
            buffer.set_buffer_size(u64::MAX as usize);
            buffer
        });
        let server_cmd = async_process::Command::new(opts.server);
        let server = sim.spawn_machine(server_cmd, None).await;
        let mut client_cmd = async_process::Command::new(opts.client);
        client_cmd.arg(server_addr.to_string());
        let client = sim.spawn_machine(client_cmd, delay).await;
        sim.plug(server, public, Some(server_addr)).await;
        match opts.topology.as_str() {
            "m2" => {
                sim.plug(client, public, None).await;
            }
            "m1m1" => {
                let public2 = sim.spawn_network(Ipv4Range::global().split(2)[1]);
                sim.plug(client, public2, None).await;
                sim.add_route(public, public2);
            }
            "m1nm1" => {
                let private = sim.spawn_network(Ipv4Range::local_subnet_192(0));
                sim.plug(client, private, None).await;
                sim.add_nat_route(NatConfig::default(), public, private);
            }
            _ => panic!("unsupported topology"),
        }
        let (server, client) = sim.machines_mut().split_at_mut(1);
        let server = &mut server[0];
        let client = &mut client[0];
        server.send(Command::Start);
        loop {
            if server.recv().await == Some(Event::Started) {
                break;
            }
        }
        client.send(Command::Start);
        loop {
            if client.recv().await == Some(Event::Started) {
                break;
            }
        }
        client.send(Command::Exit);
        loop {
            if client.recv().await == Some(Event::Exited) {
                break;
            }
        }
        server.send(Command::Exit);
        loop {
            if server.recv().await == Some(Event::Exited) {
                break;
            }
        }
    });
}
