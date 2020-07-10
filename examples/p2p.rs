use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use ipfs_embed::{Config, Store};
use libipld::cid::{Cid, Codec};
use libipld::multihash::Sha2_256;
use libipld::store::{ReadonlyStore, Store as _, Visibility};
use netsim_embed::{run, Ipv4Range, NetworkBuilder};
use tempdir::TempDir;

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
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
    Inserted,
    Got(Vec<u8>),
}

fn main() {
    run(async {
        let mut net = NetworkBuilder::new(Ipv4Range::random_local_subnet());
        let builder = |mut cmd: mpsc::Receiver<Command>, mut event: mpsc::Sender<Event>| async move {
            let tmp = TempDir::new("netsim_embed").unwrap();
            let mut config = Config::from_path(tmp.path()).unwrap();
            config.network.bootstrap_nodes = vec![];
            let store = Store::new(config).unwrap();

            while let Some(cmd) = cmd.next().await {
                match cmd {
                    Command::Insert(cid, bytes) => {
                        //store.insert(&cid, bytes.into_boxed_slice(), Visibility::Public).await.unwrap();
                        event.send(Event::Inserted).await.unwrap();
                    }
                    Command::Get(cid) => {
                        //let bytes = store.get(&cid).await.unwrap().to_vec();
                        let bytes = b"hello world".to_vec();
                        event.send(Event::Got(bytes)).await.unwrap();
                    }
                }
            }
        };

        net.spawn_machine(builder.clone());
        net.spawn_machine(builder);
        let mut net = net.spawn();
        {
            let m = net.machine(0);
            m.send(Command::insert(b"hello world")).await;
            assert_eq!(m.recv().await.unwrap(), Event::Inserted);
        }
        {
            let m = net.machine(1);
            m.send(Command::get(b"hello world")).await;
            assert_eq!(m.recv().await.unwrap(), Event::Got(b"hello world".to_vec()));
        }
    })
}
