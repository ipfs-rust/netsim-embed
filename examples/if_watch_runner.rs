use async_process::Command;
use async_std::future::timeout;
use netsim_embed::{run, Ipv4Range, Machine, Namespace, Netsim};
use std::{
    collections::BTreeSet, iter::FromIterator, net::Ipv4Addr, path::PathBuf, time::Duration,
};

async fn ping(ns: Namespace, addr: Ipv4Addr) {
    Command::new("nsenter")
        .args([
            format!("--net={}", ns),
            "ping".to_owned(),
            "-c".to_owned(),
            4.to_string(),
            "-i".to_owned(),
            "0.1".to_string(),
            addr.to_string(),
        ])
        .status()
        .await
        .unwrap();
}

fn exe(name: &str) -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join(name)
}

async fn recv(machine: &mut Machine<String, String>, mut n: usize) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    while n > 0 {
        match timeout(Duration::from_secs(3), machine.recv()).await {
            Ok(Some(s)) => {
                if s.ends_with("/32") {
                    // ignore: intermediate state between setting addr and netmask
                } else {
                    n -= 1;
                    result.insert(s);
                }
            }
            Ok(None) => panic!("machine exited"),
            Err(e) => panic!("error: {}", e),
        }
    }
    result
}

fn main() {
    env_logger::init();
    run(async {
        let mut sim = Netsim::<String, String>::new();
        let net = sim.spawn_network(Ipv4Range::global());
        let addr1 = sim.network_mut(net).unique_addr();
        let addr2 = sim.network_mut(net).unique_addr();
        let addr3 = sim.network_mut(net).unique_addr();
        let if_watch_bin = exe("if_watch");
        let wait_for_exit_bin = exe("wait_for_exit");
        let watcher = sim
            .spawn_machine(Command::new(if_watch_bin.clone()), None)
            .await;
        let pinger = sim
            .spawn_machine(Command::new(wait_for_exit_bin), None)
            .await;
        sim.plug(watcher, net, Some(addr1)).await;
        sim.plug(pinger, net, None).await;
        assert_eq!(
            recv(sim.machine(watcher), 2).await,
            BTreeSet::from_iter(["<up 127.0.0.1/8".to_owned(), format!("<up {}/0", addr1)])
        );
        ping(sim.machine(pinger).namespace(), addr1).await;
        sim.plug(watcher, net, Some(addr2)).await;
        assert_eq!(
            recv(sim.machine(watcher), 1).await,
            BTreeSet::from_iter([format!("<down {}/0", addr1),])
        );
        assert_eq!(
            recv(sim.machine(watcher), 1).await,
            BTreeSet::from_iter([format!("<up {}/0", addr2),])
        );
        ping(sim.machine(pinger).namespace(), addr2).await;
        sim.plug(watcher, net, Some(addr3)).await;
        assert_eq!(
            recv(sim.machine(watcher), 1).await,
            BTreeSet::from_iter([format!("<down {}/0", addr2),])
        );
        assert_eq!(
            recv(sim.machine(watcher), 1).await,
            BTreeSet::from_iter([format!("<up {}/0", addr3),])
        );
    });
}
