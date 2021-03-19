use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use ipfs_embed::{Config, Ipfs, Multiaddr, PeerId};
use libipld::multihash::{Code, MultihashDigest};
use libipld::raw::RawCodec;
use libipld::store::DefaultParams;
use libipld::{Block, Cid};
use netsim_embed::{run, Ipv4Range, NatConfig, NetworkBuilder};
use std::time::Duration;
use tempdir::TempDir;

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Bootstrap(Vec<(PeerId, Multiaddr)>),
    Insert(Cid, Vec<u8>),
    Get(Cid),
}

impl Command {
    pub fn insert(bytes: &[u8]) -> Self {
        let hash = Code::Blake3_256.digest(&bytes);
        let cid = Cid::new_v1(RawCodec.into(), hash);
        Self::Insert(cid, bytes.to_vec())
    }

    pub fn get(bytes: &[u8]) -> Self {
        let hash = Code::Blake3_256.digest(&bytes);
        let cid = Cid::new_v1(RawCodec.into(), hash);
        Self::Get(cid)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Event {
    Bootstrapped(PeerId, Multiaddr),
    Inserted,
    Got(Vec<u8>),
}

fn main() {
    run(async {
        let builder = |node_name: &'static str| {
            move |mut cmd: mpsc::Receiver<Command>, mut event: mpsc::Sender<Event>| async move {
                let tmp = TempDir::new("netsim_embed").unwrap();
                let mut config = Config::new(None, 0);
                config.network.node_name = node_name.to_string();
                config.network.enable_mdns = false;
                let store = Ipfs::<DefaultParams>::new(config).await.unwrap();
                /*let address = store
                .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
                .await
                .unwrap();*/
                let address: Multiaddr = Multiaddr::empty();
                let peer_id = store.local_peer_id().clone();

                while let Some(cmd) = cmd.next().await {
                    match cmd {
                        Command::Bootstrap(peers) => {
                            if !peers.is_empty() {
                                store.bootstrap(&peers).await.unwrap();
                            }
                            event
                                .send(Event::Bootstrapped(peer_id, address.clone()))
                                .await
                                .unwrap();
                        }
                        Command::Insert(cid, bytes) => {
                            store
                                .insert(&Block::new_unchecked(cid, bytes))
                                .unwrap()
                                .await
                                .unwrap();
                            event.send(Event::Inserted).await.unwrap();
                        }
                        Command::Get(cid) => {
                            let bytes = store.fetch(&cid).await.unwrap().into_inner().1;
                            event.send(Event::Got(bytes)).await.unwrap();
                        }
                    }
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
        if let Event::Bootstrapped(peer_id, addr) = m.recv().await.unwrap() {
            bootstrap.push((peer_id, addr));
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
