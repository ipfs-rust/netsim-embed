use futures::channel::mpsc;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use netsim_embed::*;
use std::net::{SocketAddrV4, TcpListener, TcpStream};
use std::time::Duration;

fn main() {
    run(async {
        let mut net = NetworkBuilder::new(Ipv4Range::global());
        let addr = net.spawn_machine(
            Duration::from_millis(0),
            |_: mpsc::Receiver<()>, _: mpsc::Sender<()>| async move {
                let addr = SocketAddrV4::new(0.into(), 3000);
                let listener = async_io::Async::<TcpListener>::bind(addr).unwrap();
                let incoming = listener.incoming();
                futures::pin_mut!(incoming);
                let mut stream = incoming.next().await.unwrap().unwrap();

                let mut buf = [0u8; 11];
                let len = stream.read(&mut buf).await.unwrap();
                assert_eq!(&buf[..len], &b"ping"[..]);

                println!("received ping");
                stream.write_all(b"pong").await.unwrap();
            },
        );

        let mut local = NetworkBuilder::new(Ipv4Range::random_local_subnet());
        local.spawn_machine(
            Duration::from_millis(0),
            move |_: mpsc::Receiver<()>, mut events: mpsc::Sender<()>| async move {
                let addr = SocketAddrV4::new(addr, 3000);
                let mut stream = async_io::Async::<TcpStream>::connect(addr).await.unwrap();
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
