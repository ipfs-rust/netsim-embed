use async_process::Command;
use futures::prelude::*;
use ipc_channel::ipc::{IpcOneShotServer, IpcSender};
pub use libpacket::*;
use netsim_embed_core::*;
pub use netsim_embed_core::{DelayBuffer, Ipv4Range, Protocol};
pub use netsim_embed_machine::{unshare_user, Machine, MachineId, Namespace};
use netsim_embed_nat::*;
use netsim_embed_router::*;
use std::fmt::Display;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::str::FromStr;

pub fn run<F>(f: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    unshare_user().unwrap();
    async_global_executor::block_on(f);
}

enum Connector {
    Unplugged(Plug),
    Plugged(NetworkId),
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct NetworkId(usize);

impl NetworkId {
    fn id(&self) -> usize {
        self.0 + u16::MAX as usize
    }
}

pub struct Netsim<C, E> {
    machines: Vec<Machine<C, E>>,
    plugs: Vec<Connector>,
    networks: Vec<Network>,
}

impl<C, E> Default for Netsim<C, E> {
    fn default() -> Self {
        Self {
            machines: Default::default(),
            plugs: Default::default(),
            networks: Default::default(),
        }
    }
}

impl<C, E> Netsim<C, E>
where
    C: Display + Send + 'static,
    E: FromStr + Display + Send + 'static,
    E::Err: std::fmt::Debug + Display + Send + Sync,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn machine(&mut self, id: MachineId) -> &mut Machine<C, E> {
        &mut self.machines[id.0]
    }

    pub fn machines(&self) -> &[Machine<C, E>] {
        &self.machines
    }

    pub fn machines_mut(&mut self) -> &mut [Machine<C, E>] {
        &mut self.machines
    }

    pub async fn spawn<T: 'static + Send>(&mut self, function: &str) -> (MachineId, IpcSender<T>) {
        let (server, server_name) = IpcOneShotServer::new().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(&["--netsim-embed-internal-call", function, &server_name]);
        let machine = self.spawn_machine(command, None).await;
        let (_, ipc) = async_global_executor::spawn_blocking(|| server.accept())
            .await
            .unwrap();
        (machine, ipc)
    }

    pub async fn spawn_machine(
        &mut self,
        command: Command,
        delay: Option<DelayBuffer>,
    ) -> MachineId {
        let (plug_a, plug_b) = wire();
        let plug_b = if let Some(delay) = delay {
            delay.spawn(plug_b)
        } else {
            plug_b
        };
        let id = MachineId(self.machines.len());
        let machine = Machine::new(id, plug_b, command).await;
        self.machines.push(machine);
        self.plugs.push(Connector::Unplugged(plug_a));
        id
    }

    pub fn network(&self, id: NetworkId) -> &Network {
        &self.networks[id.0]
    }

    pub fn network_mut(&mut self, id: NetworkId) -> &mut Network {
        &mut self.networks[id.0]
    }

    pub fn spawn_network(&mut self, range: Ipv4Range) -> NetworkId {
        let id = NetworkId(self.networks.len());
        self.networks.push(Network::new(id, range));
        id
    }

    pub async fn plug(&mut self, machine: MachineId, net: NetworkId, addr: Option<Ipv4Addr>) {
        if let Connector::Plugged(_) = self.plugs[machine.0] {
            log::debug!("Unplugging {}", machine);
            self.unplug(machine).await
        }
        let plug = std::mem::replace(&mut self.plugs[machine.0], Connector::Plugged(net));
        if let Connector::Unplugged(plug) = plug {
            let net = &mut self.networks[net.0];
            let addr = addr.unwrap_or_else(|| net.unique_addr());
            let mask = net.range.netmask_prefix_length();
            net.router
                .add_connection(machine.0, plug, vec![addr.into()]);
            log::debug!("Setting {}'s address to {}/{}", machine, addr, mask);
            self.machines[machine.0].set_addr(addr, mask).await;
        }
    }

    pub async fn unplug(&mut self, machine: MachineId) {
        if let Connector::Plugged(net) = self.plugs[machine.0] {
            self.plugs[machine.0] = if let Some(plug) = self.networks[net.0]
                .router
                .remove_connection(machine.0)
                .await
            {
                Connector::Unplugged(plug)
            } else {
                Connector::Shutdown
            };
        }
    }

    pub fn add_route(&mut self, net_a: NetworkId, net_b: NetworkId) {
        let (plug_a, plug_b) = wire();
        let range_a = self.networks[net_a.0].range;
        let range_b = self.networks[net_b.0].range;
        self.networks[net_a.0]
            .router
            .add_connection(net_b.id(), plug_b, vec![range_b.into()]);
        self.networks[net_b.0]
            .router
            .add_connection(net_a.id(), plug_a, vec![range_a.into()]);
    }

    pub fn enable_route(&mut self, net_a: NetworkId, net_b: NetworkId) {
        self.networks[net_a.0].router.enable_route(net_b.id());
        self.networks[net_b.0].router.enable_route(net_a.id());
    }

    pub fn disable_route(&mut self, net_a: NetworkId, net_b: NetworkId) {
        self.networks[net_a.0].router.disable_route(net_b.id());
        self.networks[net_b.0].router.disable_route(net_a.id());
    }

    pub fn add_nat_route(
        &mut self,
        config: NatConfig,
        public_net: NetworkId,
        private_net: NetworkId,
    ) {
        let (public, nat_public) = wire();
        let (nat_private, private) = wire();
        let nat_addr = self.networks[public_net.0].unique_addr();
        let nat_range = self.networks[private_net.0].range;
        let mut nat = Ipv4Nat::new(nat_public, nat_private, nat_addr, nat_range);
        nat.set_hair_pinning(config.hair_pinning);
        nat.set_symmetric(config.symmetric);
        nat.set_blacklist_unrecognized_addrs(config.blacklist_unrecognized_addrs);
        nat.set_restrict_endpoints(config.restrict_endpoints);
        for (protocol, port, local_addr) in config.forward_ports {
            nat.forward_port(port, local_addr, protocol);
        }
        async_global_executor::spawn(nat).detach();
        self.networks[public_net.0].router.add_connection(
            private_net.id(),
            public,
            vec![Ipv4Range::new(nat_addr, 32).into()],
        );
        self.networks[private_net.0].router.add_connection(
            public_net.id(),
            private,
            vec![Ipv4Range::global().into()],
        );
    }
}

#[derive(Debug)]
pub struct Network {
    id: NetworkId,
    range: Ipv4Range,
    router: Ipv4Router,
    device: u32,
}

impl Network {
    fn new(id: NetworkId, range: Ipv4Range) -> Self {
        let router = Ipv4Router::new(range.gateway_addr());
        Self {
            id,
            range,
            router,
            device: 0,
        }
    }

    pub fn id(&self) -> NetworkId {
        self.id
    }

    pub fn range(&self) -> Ipv4Range {
        self.range
    }

    pub fn unique_addr(&mut self) -> Ipv4Addr {
        let addr = self.range.address_for(self.device);
        self.device += 1;
        addr
    }
}

#[derive(Clone, Debug, Default)]
pub struct NatConfig {
    pub hair_pinning: bool,
    pub symmetric: bool,
    pub blacklist_unrecognized_addrs: bool,
    pub restrict_endpoints: bool,
    pub forward_ports: Vec<(Protocol, u16, SocketAddrV4)>,
}

#[macro_export]
macro_rules! dispatch_args {
    ( $( ($fn:path, $t:ty) ),* $(,)* ) => {{
        let mut args = std::env::args();
        args.next();
        if args.next().map(|v| v == "--netsim-embed-internal-call").unwrap_or(false) {
            let function = args.next().unwrap();
            let server_name = args.next().unwrap();
            $(
                if function == stringify!($fn) {
                    let (sender, receiver) = ipc_channel::ipc::channel::<$t>().unwrap();
                    let server_sender = ipc_channel::ipc::IpcSender::connect(server_name).unwrap();
                    server_sender.send(sender).unwrap();
                    $fn(receiver);
                    std::process::exit(0);
                }
            )*
            panic!("Got a netsim-embed internal call with an unknown function name")
        }
    }}
}
