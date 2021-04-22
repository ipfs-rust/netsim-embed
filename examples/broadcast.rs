use futures::channel::mpsc;
use futures::prelude::*;
use netsim_embed::*;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

fn main() {
    run(async {
        let mut builder = NetworkBuilder::new(Ipv4Range::random_local_subnet());
        builder.spawn_machine(
            Wire::new(),
            |_: mpsc::Receiver<()>, mut ev: mpsc::Sender<()>| async move {
                let socket =
                    async_io::Async::<UdpSocket>::bind((Ipv4Addr::UNSPECIFIED, 5353)).unwrap();
                let multicast = [224, 0, 0, 251].into();
                socket
                    .get_ref()
                    .join_multicast_v4(&multicast, &Ipv4Addr::UNSPECIFIED)
                    .unwrap();
                loop {
                    let mut buf = [0u8; 11];
                    let (len, _addr) = socket.recv_from(&mut buf).await.unwrap();
                    if &buf[..len] == b"broadcast" {
                        println!("received broadcast message");
                        break;
                    }
                }
                ev.send(()).await.unwrap();
            },
        );

        builder.spawn_machine(
            Wire::new(),
            move |mut cmd: mpsc::Receiver<()>, _: mpsc::Sender<()>| async move {
                let socket =
                    async_io::Async::<UdpSocket>::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
                let multicast = [224, 0, 0, 251].into();
                socket
                    .send_to(b"broadcast", SocketAddrV4::new(multicast, 5353))
                    .await
                    .unwrap();

                println!("sent broadcast message");
                cmd.next().await;
            },
        );

        let mut net = builder.spawn();
        net.machine(0).recv().await;
        net.machine(1).send(()).await;
    });
}
