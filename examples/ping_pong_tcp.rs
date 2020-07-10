use futures::channel::mpsc;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use netsim_embed::*;
use std::net::{SocketAddrV4, TcpListener, TcpStream};

fn main() {
    run(async {
        let mut net = NetworkBuilder::new(Ipv4Range::global());
        let addr = net.spawn_machine(|_: mpsc::Receiver<()>, _: mpsc::Sender<()>| async move {
            let listener = smol::Async::<TcpListener>::bind("0.0.0.0:3000").unwrap();
            let mut stream = listener.incoming().next().await.unwrap().unwrap();

            let mut buf = [0u8; 11];
            let len = stream.read(&mut buf).await.unwrap();
            assert_eq!(&buf[..len], &b"ping"[..]);

            println!("received ping");
            stream.write_all(b"pong").await.unwrap();
        });

        let mut local = NetworkBuilder::new(Ipv4Range::random_local_subnet());
        local.spawn_machine(
            move |_: mpsc::Receiver<()>, mut events: mpsc::Sender<()>| async move {
                let mut stream = smol::Async::<TcpStream>::connect(SocketAddrV4::new(addr, 3000))
                    .await
                    .unwrap();
                stream.write_all(b"ping").await.unwrap();

                let mut buf = [0u8; 11];
                let len = stream.read(&mut buf).await.unwrap();
                assert_eq!(&buf[..len], &b"pong"[..]);

                println!("received pong");
                events.send(()).await.unwrap();
            },
        );

        net.spawn_network(Some(NatConfig::default()), local);
        net.spawn().subnet(0).machine(0).recv().await;
    });
}
