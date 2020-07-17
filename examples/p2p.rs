use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use ipfs_embed::{Config, Store, Multiaddr, PeerId};
use libipld::cid::{Cid, Codec};
use libipld::multihash::Sha2_256;
use libipld::store::{ReadonlyStore, Store as _, Visibility};
use netsim_embed::{run, Ipv4Range, NatConfig, NetworkBuilder};
use tempdir::TempDir;
use std::time::Duration;

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Bootstrap(Vec<(Multiaddr, PeerId)>),
    Insert(Cid, Vec<u8>),
    Get(Cid),
}

impl Command {
    pub fn insert(bytes: &[u8]) -> Self {
        let hash = Sha2_256::digest(&bytes);
        let cid = Cid::new_v1(Codec::Raw, hash);
        Self::Insert(cid, bytes.to_vec())
    }

    pub fn get(bytes: &[u8]) -> Self {
        let hash = Sha2_256::digest(&bytes);
        let cid = Cid::new_v1(Codec::Raw, hash);
        Self::Get(cid)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Event {
    Bootstrapped(Multiaddr, PeerId),
    Inserted,
    Got(Vec<u8>),
}

fn main() {
    run(async {
        let builder = |node_name: &'static str| move |mut cmd: mpsc::Receiver<Command>, mut event: mpsc::Sender<Event>| async move {
            let tmp = TempDir::new("netsim_embed").unwrap();
            let mut config = Config::from_path(tmp.path()).unwrap();
            let bootstrap = if let Command::Bootstrap(bootstrap) = cmd.next().await.unwrap() {
                bootstrap
            } else {
                unreachable!()
            };
            config.network.node_name = node_name.to_string();
            config.network.boot_nodes = bootstrap;
            config.network.enable_mdns = false;
            let store = Store::new(config).unwrap();
            let address = store.address().clone();
            let peer_id = store.peer_id().clone();
            event.send(Event::Bootstrapped(address, peer_id)).await.unwrap();

            while let Some(cmd) = cmd.next().await {
                match cmd {
                    Command::Insert(cid, bytes) => {
                        store.insert(&cid, bytes.into_boxed_slice(), Visibility::Public).await.unwrap();
                        event.send(Event::Inserted).await.unwrap();
                    }
                    Command::Get(cid) => {
                        let bytes = store.get(&cid).await.unwrap().to_vec();
                        event.send(Event::Got(bytes)).await.unwrap();
                    }
                    _ => unreachable!(),
                }
            }
        };

        //let ranges = Ipv4Range::global().split(3);
        let mut local1 = NetworkBuilder::new(/*ranges[0]*/ Ipv4Range::random_local_subnet());
        local1.spawn_machine(builder("local1"));

        let mut local2 = NetworkBuilder::new(/*ranges[1]*/ Ipv4Range::random_local_subnet());
        local2.spawn_machine(builder("local2"));

        let natconfig = NatConfig::default();
        //natconfig.hair_pinning = true;
        //natconfig.symmetric = true;
        let mut net = NetworkBuilder::new(Ipv4Range::global());
        net.spawn_machine(builder("boot"));
        net.spawn_network(Some(natconfig), local1);
        net.spawn_network(Some(natconfig), local2);
        //net.spawn_machine(builder("local2"));

        let mut net = net.spawn();
        let mut bootstrap = vec![];

        let m = net.machine(0);
        m.send(Command::Bootstrap(vec![])).await;
        if let Event::Bootstrapped(addr, peer_id) = m.recv().await.unwrap() {
            bootstrap.push((addr, peer_id));
        } else {
            unreachable!()
        }

        // wait for bootstrap node to start up (run in release mode).
        smol::Timer::after(Duration::from_millis(500)).await;

        let m = net.subnet(0).machine(0);
        m.send(Command::Bootstrap(bootstrap.clone())).await;
        m.recv().await.unwrap();

        // wait for bootstrap to complete (run in release mode).
        smol::Timer::after(Duration::from_millis(500)).await;

        m.send(Command::insert(b"hello world")).await;
        assert_eq!(m.recv().await.unwrap(), Event::Inserted);

        smol::Timer::after(Duration::from_millis(500)).await;

        let m = net.subnet(1).machine(0);
        m.send(Command::Bootstrap(bootstrap)).await;
        m.recv().await.unwrap();

        // wait for bootstrap to complete (run in release mode).
        smol::Timer::after(Duration::from_millis(500)).await;

        m.send(Command::get(b"hello world")).await;
        assert_eq!(m.recv().await.unwrap(), Event::Got(b"hello world".to_vec()));
    })
}
