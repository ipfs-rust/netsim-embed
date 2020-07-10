use futures::channel::mpsc;
use futures::sink::SinkExt;
use netsim_embed::*;
use std::net::{SocketAddrV4, UdpSocket};

fn main() {
    run(async {
        let mut net = NetworkBuilder::new(Ipv4Range::global());
        let addr = net.spawn_machine(|_: mpsc::Receiver<()>, _: mpsc::Sender<()>| async move {
            let socket = smol::Async::<UdpSocket>::bind("0.0.0.0:3000").unwrap();
            loop {
                let mut buf = [0u8; 11];
                let (len, addr) = socket.recv_from(&mut buf).await.unwrap();
                if &buf[..len] == b"ping" {
                    println!("received ping");

                    socket.send_to(b"pong", addr).await.unwrap();
                    break;
                }
            }
        });

        let mut local = NetworkBuilder::new(Ipv4Range::random_local_subnet());
        local.spawn_machine(move |_: mpsc::Receiver<()>, mut events: mpsc::Sender<()>| async move {
            let socket = smol::Async::<UdpSocket>::bind("0.0.0.0:0").unwrap();
            socket
                .send_to(b"ping", SocketAddrV4::new(addr, 3000))
                .await
                .unwrap();

            let mut buf = [0u8; 11];
            let (len, _addr) = socket.recv_from(&mut buf).await.unwrap();
            if &buf[..len] == b"pong" {
                println!("received pong");
                events.send(()).await.unwrap();
            }
        });

        net.spawn_network(Some(NatConfig::default()), local);
        net.spawn().subnet(0).machine(0).recv().await;
    });
}
