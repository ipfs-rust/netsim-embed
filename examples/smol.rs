use netsim_embed::{machine, namespace, wire, Ipv4Nat, Ipv4Range, Ipv4Router};
use std::net::{SocketAddrV4, UdpSocket};

fn main() {
    env_logger::init();
    namespace::unshare_user().unwrap();
    let range_private = Ipv4Range::new("192.168.1.0".parse().unwrap(), 24);
    let range_public = Ipv4Range::new("8.8.8.0".parse().unwrap(), 24);
    let addr_client = "192.168.1.5".parse().unwrap();
    let addr_server = "8.8.8.4".parse().unwrap();
    let addr_nat = "8.8.8.8".parse().unwrap();
    let (plug_nat_private, plug_client) = wire();
    let (plug_router_1, plug_nat_public) = wire();
    let (plug_router_2, plug_server) = wire();

    let server = machine(addr_server, 24, plug_server, async move {
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

    let client = machine(addr_client, 24, plug_client, async move {
        let socket = smol::Async::<UdpSocket>::bind("0.0.0.0:0").unwrap();
        socket
            .send_to(b"ping", SocketAddrV4::new(addr_server, 3000))
            .await
            .unwrap();

        let mut buf = [0u8; 11];
        let (len, _addr) = socket.recv_from(&mut buf).await.unwrap();
        if &buf[..len] == b"pong" {
            println!("received pong");
        }
    });

    let nat = Ipv4Nat::new(plug_nat_public, plug_nat_private, addr_nat, range_private);
    smol::Task::spawn(nat).detach();

    let mut router = Ipv4Router::new(range_public.gateway_addr());
    router.add_connection(plug_router_1, vec![addr_nat.into()]);
    router.add_connection(plug_router_2, vec![addr_server.into()]);
    smol::Task::spawn(router).detach();

    server.join().unwrap();
    client.join().unwrap();
}
