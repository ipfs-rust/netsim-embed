use futures::channel::mpsc;
use futures::sink::SinkExt;
use netsim_embed::*;
use std::io::IoSliceMut;
use std::net::SocketAddrV4;
use udp_socket::{RecvMeta, Transmit, UdpSocket};

fn main() {
    run(async {
        let mut net = NetworkBuilder::new(Ipv4Range::global());
        let addr = net.spawn_machine(
            Wire::new(),
            |_: mpsc::UnboundedReceiver<()>, mut events: mpsc::UnboundedSender<()>| async move {
                let addr = SocketAddrV4::new(0.into(), 3000);
                let socket = UdpSocket::bind(addr.into()).unwrap();
                let mut meta = [RecvMeta::default()];
                let mut data = [0; 16];
                let mut buffer = [IoSliceMut::new(&mut data[..])];
                for _ in 0..3 {
                    println!("receiving");
                    socket.recv(&mut buffer, &mut meta).await.unwrap();
                    let slice = &buffer[0][..meta[0].len];
                    println!("received {:?}", slice);
                }
                events.send(()).await.unwrap();
            },
        );
        net.spawn_machine(
            Wire::new(),
            move |_: mpsc::UnboundedReceiver<()>, mut events: mpsc::UnboundedSender<()>| async move {
                let laddr = SocketAddrV4::new(0.into(), 3000);
                let socket = UdpSocket::bind(laddr.into()).unwrap();
                let transmit = Transmit {
                    destination: SocketAddrV4::new(addr, 3000).into(),
                    contents: [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2].to_vec(),
                    segment_size: Some(4),
                    ecn: None,
                    src_ip: None,
                };
                socket.send(&[transmit]).await.unwrap();
                println!("sent");
                events.send(()).await.unwrap();
            },
        );
        let mut net = net.spawn();
        net.machine(0).recv().await;
        net.machine(1).recv().await;
    });
}
