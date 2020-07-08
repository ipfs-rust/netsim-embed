use smol_netsim::{machine, namespace, wire, Ipv4Range, Ipv4Router};
use std::net::{SocketAddrV4, UdpSocket};

fn main() {
    env_logger::init();
    namespace::unshare_user().unwrap();
    let range = Ipv4Range::new("192.168.1.0".parse().unwrap(), 24);
    let a_addr = "192.168.1.5".parse().unwrap();
    let b_addr = "192.168.1.6".parse().unwrap();
    let (r1, a) = wire();
    let (r2, b) = wire();

    let join1 = machine(a_addr, 24, a, async move {
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

    let mut router = Ipv4Router::new(range.gateway_addr());
    router.add_connection(r1, vec![a_addr.into()]);
    router.add_connection(r2, vec![b_addr.into()]);
    smol::Task::spawn(router).detach();

    join1.join().unwrap();
    join2.join().unwrap();
}
