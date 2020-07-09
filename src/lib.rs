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

    pub fn range(&self) -> Ipv4Range {
        Ipv4Range::new(self.addr, self.mask)
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

pub fn nat(config: NatConfig, range: Ipv4Range, plug: RoutablePlug) -> RoutablePlug {
    let (a, b) = wire();
    let public_ip = range.random_client_addr();
    let private_range = plug.range();
    let mut nat = Ipv4Nat::new(b, plug.plug, public_ip, private_range);
    nat.set_hair_pinning(config.hair_pinning);
    nat.set_symmetric(config.symmetric);
    nat.set_blacklist_unrecognized_addrs(config.blacklist_unrecognized_addrs);
    nat.set_restrict_endpoints(config.restrict_endpoints);
    smol::Task::spawn(nat).detach();
    RoutablePlug {
        plug: a,
        addr: public_ip,
        mask: 32,
        router: false,
        tasks: plug.tasks,
    }
}

pub fn router(range: Ipv4Range, mut plugs: Vec<RoutablePlug>) -> RoutablePlug {
    if plugs.len() < 2 {
        return plugs.remove(0);
    }
    let (a, b) = wire();
    let addr = range.gateway_addr();
    let mask = range.netmask_prefix_length();

    let mut router = Ipv4Router::new(addr);
    let mut tasks = vec![];
    for plug in plugs {
        let route = plug.to_route();
        router.add_connection(plug.plug, vec![route]);
        tasks.extend(plug.tasks);
    }
    let plug = RoutablePlug {
        plug: a,
        addr,
        mask,
        router: true,
        tasks,
    };
    router.add_connection(b, vec![plug.to_route()]);
    smol::Task::spawn(router).detach();
    plug
}

#[derive(Clone, Default)]
pub struct StarConfig {
    pub nat_config: NatConfig,
    pub num_public: u8,
    pub num_nat: u8,
    pub num_private: u8,
}

pub fn star<B, F>(config: StarConfig, builder: B) -> RoutablePlug
where
    B: Fn(u32, u32) -> F,
    F: Future<Output = ()> + Send + 'static,
{
    let mut peers = vec![];
    for _ in 0..config.num_nat {
        let mut local_peers = vec![];
        let subnet = Ipv4Range::random_local_subnet();
        for _ in 0..config.num_private {
            local_peers.push(machine(subnet, builder()));
        }
        let router = router(subnet, local_peers);
        let nat = nat(config.nat_config, Ipv4Range::global(), router);
        peers.push(nat);
    }
    for _ in 0..config.num_public {
        peers.push(machine(Ipv4Range::global(), builder()));
    }
    router(Ipv4Range::global(), peers)
}

pub fn run_star<B, F>(config: StarConfig, builder: B)
where
    B: Fn() -> F,
    F: Future<Output = ()> + Send + 'static,
{
    run_star(async move {
        star(config, builder)
    })
}
