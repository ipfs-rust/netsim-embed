use futures::channel::mpsc;
use if_watch::{IfEvent, IfWatcher};
use ipnet::{IpNet, Ipv4Net};
use netsim_embed::*;

fn main() {
    run(async {
        let mut net = NetworkBuilder::new(Ipv4Range::global());
        for _ in 0..100 {
            net.spawn_machine(
                Wire::new(),
                |_: mpsc::UnboundedReceiver<()>, ev: mpsc::UnboundedSender<IfEvent>| async move {
                    let mut watcher = IfWatcher::new().await.unwrap();
                    loop {
                        let watcher = &mut watcher;
                        let event = watcher.await.unwrap();
                        if let IfEvent::Up(IpNet::V4(_)) = &event {
                            ev.unbounded_send(event).unwrap();
                            break;
                        }
                    }
                },
            );
        }

        let mut net = net.spawn();
        for machine in net.machines_mut() {
            let ev = machine.recv().await;
            assert_eq!(Some(IfEvent::Up(IpNet::V4(Ipv4Net::new(machine.addr(), 0).unwrap()))), ev);
        }
    });
}
