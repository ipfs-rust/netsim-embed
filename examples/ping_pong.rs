use netsim_embed::*;
use std::net::{SocketAddrV4, UdpSocket};

fn main() {
    run(async {
        let server = machine(Ipv4Range::global(), async move {
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

        let server_addr = server.addr();
        let client = machine(Ipv4Range::random_local_subnet(), async move {
            let socket = smol::Async::<UdpSocket>::bind("0.0.0.0:0").unwrap();
            socket
                .send_to(b"ping", SocketAddrV4::new(server_addr, 3000))
                .await
                .unwrap();

            let mut buf = [0u8; 11];
            let (len, _addr) = socket.recv_from(&mut buf).await.unwrap();
            if &buf[..len] == b"pong" {
                println!("received pong");
            }
        });

        let nat = nat(NatConfig::default(), Ipv4Range::global(), client);
        router(Ipv4Range::global(), vec![nat, server])
    });
}
