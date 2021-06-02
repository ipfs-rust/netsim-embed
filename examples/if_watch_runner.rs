use async_process::Command;
use netsim_embed::{run, Ipv4Range, Namespace, Netsim};
use std::net::Ipv4Addr;
use std::path::PathBuf;

async fn ping(ns: Namespace, addr: Ipv4Addr) {
    let mut cmd = Command::new("nsenter");
    cmd.arg(format!("--net={}", ns));
    cmd.arg("ping");
    cmd.arg("-c").arg(4.to_string()).arg(addr.to_string());
    cmd.status().await.unwrap();
}

fn exe(name: &str) -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join(name)
}

fn main() {
    env_logger::init();
    run(async {
        let mut sim = Netsim::<String, String>::new();
        let net = sim.spawn_network(Ipv4Range::global());
        let addr1 = sim.network_mut(net).random_addr();
        let addr2 = sim.network_mut(net).random_addr();
        let addr3 = sim.network_mut(net).random_addr();
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
            sim.machine(watcher).recv().await,
            Some("<up 127.0.0.1/8".into())
        );
        assert_eq!(
            sim.machine(watcher).recv().await,
            Some(format!("<up {}/0", addr1))
        );
        ping(sim.machine(pinger).namespace(), addr1).await;
        sim.plug(watcher, net, Some(addr2)).await;
        assert_eq!(
            sim.machine(watcher).recv().await,
            Some(format!("<down {}/0", addr1))
        );
        assert_eq!(
            sim.machine(watcher).recv().await,
            Some(format!("<up {}/32", addr2))
        );
        assert_eq!(
            sim.machine(watcher).recv().await,
            Some(format!("<down {}/32", addr2))
        );
        assert_eq!(
            sim.machine(watcher).recv().await,
            Some(format!("<up {}/0", addr2))
        );
        ping(sim.machine(pinger).namespace(), addr2).await;
        sim.plug(watcher, net, Some(addr3)).await;
        assert_eq!(
            sim.machine(watcher).recv().await,
            Some(format!("<down {}/0", addr2))
        );
        assert_eq!(
            sim.machine(watcher).recv().await,
            Some(format!("<up {}/32", addr3))
        );
        assert_eq!(
            sim.machine(watcher).recv().await,
            Some(format!("<down {}/32", addr3))
        );
        assert_eq!(
            sim.machine(watcher).recv().await,
            Some(format!("<up {}/0", addr3))
        );
    });
}
