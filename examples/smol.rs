use smol_netsim::{machine, namespace, wire};
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

fn main() {
    namespace::unshare_user().unwrap();
    let a_addr: Ipv4Addr = "192.168.1.5".parse().unwrap();
    let b_addr = "192.168.1.6".parse().unwrap();
    let (a, b) = wire();

    let join1 = machine(a_addr.clone(), 24, a, async move {
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

    let join2 = machine(b_addr, 24, b, async move {
        let socket = smol::Async::<UdpSocket>::bind("0.0.0.0:3000").unwrap();
        socket
            .send_to(b"ping", SocketAddrV4::new(a_addr, 3000))
            .await
            .unwrap();

        let mut buf = [0u8; 11];
        let (len, _addr) = socket.recv_from(&mut buf).await.unwrap();
        if &buf[..len] == b"pong" {
            println!("received pong");
        }
    });

    join1.join().unwrap();
    join2.join().unwrap();
}
