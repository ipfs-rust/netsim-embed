use async_process::Command;
use futures::prelude::*;
pub use libpacket::*;
use netsim_embed_core::*;
pub use netsim_embed_core::{Ipv4Range, Wire};
pub use netsim_embed_machine::unshare_user;
use netsim_embed_machine::*;
use netsim_embed_nat::*;
use netsim_embed_router::*;
use std::fmt::Display;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn run<F>(f: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    unshare_user().unwrap();
    async_global_executor::block_on(f);
}

fn id() -> u64 {
    static ID: AtomicU64 = AtomicU64::new(0);
    ID.fetch_add(1, Ordering::SeqCst)
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
pub struct Network<C, E> {
    id: u64,
    range: Ipv4Range,
    router: Ipv4Router,
    machines: Vec<Machine<C, E>>,
    networks: Vec<Network<C, E>>,
}

impl<C, E> Network<C, E>
where
    C: Display + Send + 'static,
    E: FromStr + Send + 'static,
    E::Err: std::fmt::Debug + Display + Send + Sync,
{
    pub fn new(range: Ipv4Range) -> Self {
        let router = Ipv4Router::new(range.gateway_addr());
        Self {
            id: id(),
            range,
            router,
            machines: Default::default(),
            networks: Default::default(),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

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

    pub fn random_client_addr(&self) -> Ipv4Addr {
        self.range.random_client_addr()
    }

    pub async fn spawn_machine(&mut self, config: Wire, addr: Option<Ipv4Addr>, command: Command) {
        let (iface_a, iface_b) = config.spawn();
        let addr = addr.unwrap_or_else(|| self.random_client_addr());
        let mask = self.range.netmask_prefix_length();
        let id = id();
        let machine = Machine::new(id, addr, mask, iface_b, command).await;
        self.machines.push(machine);
        self.router.add_connection(id, iface_a, vec![addr.into()]);
    }

    pub fn spawn_network(&mut self, config: Option<NatConfig>, mut net: Network<C, E>) {
        let (net_a, net_b) = wire();
        if let Some(config) = config {
            net.router
                .add_connection(self.id, net_b, vec![Ipv4Range::global().into()]);
            let (nat_a, nat_b) = wire();
            let nat_addr = self.range.random_client_addr();
            let mut nat = Ipv4Nat::new(nat_b, net_a, nat_addr, net.range);
            nat.set_hair_pinning(config.hair_pinning);
            nat.set_symmetric(config.symmetric);
            nat.set_blacklist_unrecognized_addrs(config.blacklist_unrecognized_addrs);
            nat.set_restrict_endpoints(config.restrict_endpoints);
            async_global_executor::spawn(nat).detach();
            self.router
                .add_connection(net.id(), nat_a, vec![nat_addr.into()]);
        } else {
            net.router
                .add_connection(self.id, net_b, vec![Ipv4Range::global().into()]);
            self.router
                .add_connection(net.id(), net_a, vec![net.range.into()]);
        }
        self.networks.push(net);
    }
}
