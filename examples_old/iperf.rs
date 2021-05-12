use async_process::Command;
use futures::channel::mpsc;
use futures::prelude::*;
use netsim_embed::*;
use std::time::Duration;

fn main() {
    run(async {
        let mut net = NetworkBuilder::new(Ipv4Range::global());
        let addr = net.spawn_machine(
            Wire::new(),
            |mut cmd: mpsc::UnboundedReceiver<()>, _: mpsc::UnboundedSender<()>| async move {
                let mut child = Command::new("iperf")
                    .arg("-s")
                    .arg("-w")
                    .arg("1M")
                    .arg("-m")
                    .spawn()
                    .unwrap();
                cmd.next().await;
                child.kill().unwrap();
            },
        );

        let mut wire = Wire::new();
        wire.set_delay(Duration::from_millis(10));
        wire.set_buffer_size(u64::MAX as usize);
        net.spawn_machine(
            wire,
            move |_: mpsc::UnboundedReceiver<()>, mut events: mpsc::UnboundedSender<()>| async move {
                Command::new("iperf")
                    .arg("-c")
                    .arg(format!("{}", addr))
                    .arg("-w")
                    .arg("1M")
                    .arg("-m")
                    .spawn()
                    .unwrap()
                    .status()
                    .await
                    .unwrap();
                Command::new("netstat")
                    .arg("-s")
                    .spawn()
                    .unwrap()
                    .status()
                    .await
                    .unwrap();
                events.send(()).await.unwrap();
            },
        );

        let mut net = net.spawn();
        net.machine(1).recv().await;
        net.machine(0).send(()).await;
    });
}
