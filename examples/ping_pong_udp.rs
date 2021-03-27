use futures::channel::mpsc;
use futures::sink::SinkExt;
use netsim_embed::*;
use std::net::{SocketAddrV4, UdpSocket};

fn main() {
    run(async {
        let mut net = NetworkBuilder::new(Ipv4Range::global());
        let addr = net.spawn_machine(
            Wire::new(),
            |_: mpsc::Receiver<()>, _: mpsc::Sender<()>| async move {
                let addr = SocketAddrV4::new(0.into(), 3000);
                let socket = async_io::Async::<UdpSocket>::bind(addr).unwrap();
                loop {
                    let mut buf = [0u8; 11];
                    let (len, addr) = socket.recv_from(&mut buf).await.unwrap();
                    if &buf[..len] == b"ping" {
                        println!("received ping");

                        socket.send_to(b"pong", addr).await.unwrap();
                        break;
                    }
                }
            },
        );

        let mut local = NetworkBuilder::new(Ipv4Range::random_local_subnet());
        local.spawn_machine(
            Wire::new(),
            move |_: mpsc::Receiver<()>, mut events: mpsc::Sender<()>| async move {
                let laddr = SocketAddrV4::new(0.into(), 3000);
                let socket = async_io::Async::<UdpSocket>::bind(laddr).unwrap();
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
            },
        );

        net.spawn_network(Some(NatConfig::default()), local);
        net.spawn().subnet(0).machine(0).recv().await;
    });
}
