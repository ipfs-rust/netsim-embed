pub use netsim_embed_core::Ipv4Range;
use netsim_embed_core::*;
use netsim_embed_machine::namespace;
use netsim_embed_nat::*;
use netsim_embed_router::*;
pub use pnet_packet::*;
use std::future::Future;
use std::net::Ipv4Addr;

pub fn run<F>(f: F)
where
    F: Future<Output = RoutablePlug> + Send + 'static,
{
    env_logger::init();
    namespace::unshare_user().unwrap();
    smol::run(async move {
        let plug = f.await;
        for task in plug.tasks {
            task.await;
        }
    });
}

#[derive(Debug)]
pub struct RoutablePlug {
    plug: Plug,
    addr: Ipv4Addr,
    mask: u8,
    router: bool,
    tasks: Vec<smol::Task<()>>,
}

impl RoutablePlug {
    pub fn addr(&self) -> Ipv4Addr {
        self.addr
    }

    fn to_route(&self) -> Ipv4Route {
        if self.router {
            Ipv4Range::new(self.addr, self.mask).into()
        } else {
            self.addr.into()
        }
    }
}

pub fn machine<F>(range: Ipv4Range, task: F) -> RoutablePlug
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let (a, b) = wire();
    let addr = range.random_client_addr();
    let mask = range.netmask_prefix_length();
    let task = smol::Task::blocking(async move {
        let join = netsim_embed_machine::machine(addr, mask, b, task);
        join.join().unwrap();
    });
    RoutablePlug {
        plug: a,
        addr,
        mask,
        router: false,
        tasks: vec![task],
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NatConfig {
    pub public_ip: Ipv4Addr,
    pub hair_pinning: bool,
    pub symmetric: bool,
    pub blacklist_unrecognized_addrs: bool,
    pub restrict_endpoints: bool,
}

impl Default for NatConfig {
    fn default() -> Self {
        Self {
            public_ip: Ipv4Range::global().random_client_addr(),
            hair_pinning: false,
            symmetric: false,
            blacklist_unrecognized_addrs: false,
            restrict_endpoints: false,
        }
    }
}

pub fn nat(config: NatConfig, plug: RoutablePlug) -> RoutablePlug {
    let (a, b) = wire();
    let private_range = Ipv4Range::new(plug.addr, plug.mask);
    let mut nat = Ipv4Nat::new(b, plug.plug, config.public_ip, private_range);
    nat.set_hair_pinning(config.hair_pinning);
    nat.set_symmetric(config.symmetric);
    nat.set_blacklist_unrecognized_addrs(config.blacklist_unrecognized_addrs);
    nat.set_restrict_endpoints(config.restrict_endpoints);
    smol::Task::spawn(nat).detach();
    RoutablePlug {
        plug: a,
        addr: config.public_ip,
        mask: 32,
        router: false,
        tasks: plug.tasks,
    }
}

pub fn router(range: Ipv4Range, mut plug1: RoutablePlug, plug2: RoutablePlug) -> RoutablePlug {
    let (a, b) = wire();
    let addr = range.gateway_addr();
    let mask = range.netmask_prefix_length();
    let route1 = plug1.to_route();
    let route2 = plug2.to_route();
    plug1.tasks.extend(plug2.tasks);
    let plug = RoutablePlug {
        plug: a,
        addr,
        mask,
        router: true,
        tasks: plug1.tasks,
    };
    let mut router = Ipv4Router::new(addr);
    router.add_connection(plug1.plug, vec![route1]);
    router.add_connection(plug2.plug, vec![route2]);
    router.add_connection(b, vec![plug.to_route()]);
    smol::Task::spawn(router).detach();
    plug
}
