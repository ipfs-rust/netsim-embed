use async_process::Command;
use futures::channel::mpsc;
use futures::prelude::*;
use netsim_embed_core::*;
pub use netsim_embed_core::{Ipv4Range, Wire};
pub use netsim_embed_machine::namespace;
use netsim_embed_machine::*;
use netsim_embed_nat::*;
use netsim_embed_router::*;
pub use pnet_packet::*;
use std::fmt::Display;
use std::net::Ipv4Addr;
use std::str::FromStr;

pub fn run<F>(f: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    namespace::unshare_user().unwrap();
    async_global_executor::block_on(f);
}

#[derive(Debug)]
pub struct Machine<C, E> {
    addr: Ipv4Addr,
    tx: mpsc::UnboundedSender<C>,
    rx: mpsc::UnboundedReceiver<E>,
}

impl<C: Send + 'static, E: Send + 'static> Machine<C, E> {
    pub fn addr(&self) -> Ipv4Addr {
        self.addr
    }

    pub async fn send(&mut self, cmd: C) {
        self.tx.send(cmd).await.unwrap();
    }

    pub async fn recv(&mut self) -> Option<E> {
        self.rx.next().await
    }
}

#[derive(Debug)]
pub struct Network<C, E> {
    range: Ipv4Range,
    machines: Vec<Machine<C, E>>,
    networks: Vec<Network<C, E>>,
}

impl<C: Send + 'static, E: Send + 'static> Network<C, E> {
    pub fn range(&self) -> Ipv4Range {
        self.range
    }

    pub fn subnet(&mut self, i: usize) -> &mut Network<C, E> {
        self.networks.get_mut(i).unwrap()
    }

    pub fn subnets(&self) -> &[Network<C, E>] {
        &self.networks
    }

    pub fn subnets_mut(&mut self) -> &mut [Network<C, E>] {
        &mut self.networks
    }

    pub fn machine(&mut self, i: usize) -> &mut Machine<C, E> {
        self.machines.get_mut(i).unwrap()
    }

    pub fn machines(&self) -> &[Machine<C, E>] {
        &self.machines
    }

    pub fn machines_mut(&mut self) -> &mut [Machine<C, E>] {
        &mut self.machines
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NatConfig {
    pub hair_pinning: bool,
    pub symmetric: bool,
    pub blacklist_unrecognized_addrs: bool,
    pub restrict_endpoints: bool,
}

impl Default for NatConfig {
    fn default() -> Self {
        Self {
            hair_pinning: false,
            symmetric: false,
            blacklist_unrecognized_addrs: false,
            restrict_endpoints: false,
        }
    }
}

#[derive(Debug)]
pub struct NetworkBuilder<C, E> {
    range: Ipv4Range,
    router: Ipv4Router,
    machines: Vec<Machine<C, E>>,
    networks: Vec<Network<C, E>>,
}

impl<C, E> NetworkBuilder<C, E>
where
    C: Display + Send + 'static,
    E: FromStr + Send + 'static,
    E::Err: std::fmt::Debug + Send,
{
    pub fn new(range: Ipv4Range) -> Self {
        let router = Ipv4Router::new(range.gateway_addr());
        Self {
            range,
            router,
            machines: Default::default(),
            networks: Default::default(),
        }
    }

    pub fn spawn_machine(&mut self, config: Wire, command: Command) -> Ipv4Addr {
        let (iface_a, iface_b) = config.spawn();
        let (cmd_tx, cmd_rx) = mpsc::unbounded();
        let (event_tx, event_rx) = mpsc::unbounded();
        let addr = self.range.random_client_addr();
        let mask = self.range.netmask_prefix_length();
        let _ = machine(addr, mask, iface_b, command, cmd_rx, event_tx);
        let machine = Machine {
            addr,
            tx: cmd_tx,
            rx: event_rx,
        };
        self.machines.push(machine);
        self.router.add_connection(iface_a, vec![addr.into()]);
        addr
    }

    pub fn spawn_network(&mut self, config: Option<NatConfig>, mut builder: NetworkBuilder<C, E>) {
        let (net_a, net_b) = wire();
        if let Some(config) = config {
            builder
                .router
                .add_connection(net_b, vec![Ipv4Range::global().into()]);
            let (nat_a, nat_b) = wire();
            let nat_addr = self.range.random_client_addr();
            let mut nat = Ipv4Nat::new(nat_b, net_a, nat_addr, builder.range);
            nat.set_hair_pinning(config.hair_pinning);
            nat.set_symmetric(config.symmetric);
            nat.set_blacklist_unrecognized_addrs(config.blacklist_unrecognized_addrs);
            nat.set_restrict_endpoints(config.restrict_endpoints);
            async_global_executor::spawn(nat).detach();
            self.router.add_connection(nat_a, vec![nat_addr.into()]);
        } else {
            builder
                .router
                .add_connection(net_b, vec![Ipv4Range::global().into()]);
            self.router
                .add_connection(net_a, vec![builder.range.into()]);
        }
        let network = builder.spawn();
        self.networks.push(network);
    }

    pub fn spawn(self) -> Network<C, E> {
        let Self {
            range,
            router,
            machines,
            networks,
        } = self;
        async_global_executor::spawn(router).detach();
        Network {
            range,
            machines,
            networks,
        }
    }
}
