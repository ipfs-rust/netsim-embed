use async_process::Command;
use futures::prelude::*;
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

    #[cfg(feature = "ipc")]
    pub async fn spawn<M: MachineFn>(&mut self, _machine: M, arg: M::Arg) -> MachineId {
        use ipc_channel::ipc;
        let id = M::id();
        let (server, server_name) = ipc::IpcOneShotServer::<ipc::IpcSender<M::Arg>>::new().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--netsim-embed-internal-call",
            &format!("{id}"),
            &server_name,
        ]);
        let machine = self.spawn_machine(command, None).await;
        let (_, ipc) = async_global_executor::spawn_blocking(|| server.accept())
            .await
            .unwrap();
        ipc.send(arg)
            .expect("Failed sending argument to child process");
        machine
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

#[cfg(feature = "ipc")]
pub trait MachineFn {
    type Arg: 'static + Send + serde::Serialize;
    fn id() -> u128;
    fn call(arg: Self::Arg);
}

#[cfg(feature = "ipc")]
pub use netsim_embed_macros::machine;

#[allow(clippy::needless_doctest_main)]
/// Dispatch spawned machine invocations to their declared functions.
///
/// Each function must be annotated with `#[no_mangle]` so that the symbol is exported,
/// and the current executable must be linked with `-rdynamic` to add these symbols to
/// the dynamic symbol table. The latter is best done with a `build.rs` like this:
///
/// ```no_run
/// fn main() {
///     println!("cargo:rustc-link-arg-tests=-rdynamic");
/// }
/// ```
#[cfg(feature = "ipc")]
#[macro_export]
macro_rules! declare_machines {
    ( $($machine:path),* ) => {{
        let mut args = std::env::args();
        args.next();
        if args.next().map(|v| v == "--netsim-embed-internal-call").unwrap_or(false) {
            let function = args.next().unwrap();
            let server_name = args.next().unwrap();
            let function: u128 = function.parse().expect("Got a non-integer function to call");
            $(
                if function == <$machine as $crate::MachineFn>::id() {
                    let (sender, receiver) = $crate::test_util::ipc::channel().unwrap();
                    let server_sender = $crate::test_util::ipc::IpcSender::connect(server_name).unwrap();
                    server_sender.send(sender).unwrap();
                    <$machine as $crate::MachineFn>::call(receiver.recv().expect("Failed receiving argument from main process"));
                    std::process::exit(0);
                }
            )*
            panic!("Got a netsim-embed internal call with an unknown function name")
        }
    }}
}

#[cfg(feature = "ipc")]
pub mod test_util {
    pub struct TestResult(anyhow::Result<()>);
    impl TestResult {
        pub fn into_inner(self) -> anyhow::Result<()> {
            self.0
        }
    }
    impl From<()> for TestResult {
        fn from(_: ()) -> Self {
            Self(Ok(()))
        }
    }
    impl<E: std::error::Error + Send + Sync + 'static> From<Result<(), E>> for TestResult {
        fn from(res: Result<(), E>) -> Self {
            Self(res.map_err(Into::into))
        }
    }
    pub use ipc_channel::ipc;
    pub use libtest_mimic::{run, Arguments, Trial};
}

#[cfg(feature = "ipc")]
#[macro_export]
macro_rules! run_tests {
    ( $($fn:path),* ) => {{
        $crate::unshare_user().unwrap();
        let args = $crate::test_util::Arguments::from_args();
        let tests = vec![
            $($crate::test_util::Trial::test(stringify!($fn), || {
                $crate::test_util::TestResult::from($fn()).into_inner()?;
                Ok(())
            })),*
        ];
        $crate::test_util::run(&args, tests).exit();
    }};
}
