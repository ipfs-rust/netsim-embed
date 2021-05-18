use futures::channel::{mpsc, oneshot};
use futures::future::{poll_fn, FutureExt};
use futures::stream::{FuturesUnordered, StreamExt};
use libpacket::ipv4::Ipv4Packet;
use netsim_embed_core::{Ipv4Route, Plug};
use std::net::Ipv4Addr;
use std::task::Poll;

#[derive(Debug)]
enum RouterCtrl {
    AddRoute(u64, Plug, Vec<Ipv4Route>),
    RemoveRoute(u64, oneshot::Sender<Option<Plug>>),
}

#[derive(Debug)]
pub struct Ipv4Router {
    addr: Ipv4Addr,
    ctrl: mpsc::UnboundedSender<RouterCtrl>,
}

impl Ipv4Router {
    pub fn new(addr: Ipv4Addr) -> Self {
        let (tx, rx) = mpsc::unbounded();
        router(addr, rx);
        Self { addr, ctrl: tx }
    }

    pub fn add_connection(&mut self, id: u64, plug: Plug, routes: Vec<Ipv4Route>) {
        self.ctrl
            .unbounded_send(RouterCtrl::AddRoute(id, plug, routes))
            .ok();
    }

    pub async fn remove_connection(&mut self, id: u64) -> Option<Plug> {
        let (tx, rx) = oneshot::channel();
        self.ctrl
            .unbounded_send(RouterCtrl::RemoveRoute(id, tx))
            .unwrap();
        rx.await.unwrap()
    }
}

fn router(addr: Ipv4Addr, mut ctrl: mpsc::UnboundedReceiver<RouterCtrl>) {
    async_global_executor::spawn(async move {
        let mut conns = vec![];
        loop {
            futures::select! {
                ctrl = ctrl.next() => match ctrl {
                    Some(RouterCtrl::AddRoute(id, plug, routes)) => {
                        conns.push((id, plug, routes));
                    }
                    Some(RouterCtrl::RemoveRoute(id, ch)) => {
                        let plug = if let Some(idx) = conns.iter().position(|(id2, _, _)| *id2 == id) {
                            let (_, plug, _) = conns.swap_remove(idx);
                            Some(plug)
                        } else {
                            None
                        };
                        ch.send(plug).ok();
                    }
                    None => break,
                },
                incoming = incoming(&mut conns).fuse() => match incoming {
                    (_, Some(packet)) => forward_packet(addr, &mut conns, packet),
                    (i, None) => { conns.swap_remove(i); }
                }
            }
        }
    }).detach()
}

async fn incoming(conns: &mut [(u64, Plug, Vec<Ipv4Route>)]) -> (usize, Option<Vec<u8>>) {
    if conns.is_empty() {
        poll_fn(|_| Poll::Pending).await
    } else {
        conns
            .iter_mut()
            .enumerate()
            .map(|(i, (_, plug, _))| async move { (i, plug.incoming().await) })
            .collect::<FuturesUnordered<_>>()
            .next()
            .await
            .unwrap()
    }
}

fn forward_packet(addr: Ipv4Addr, conns: &mut [(u64, Plug, Vec<Ipv4Route>)], bytes: Vec<u8>) {
    let packet = if let Some(packet) = Ipv4Packet::new(&bytes) {
        packet
    } else {
        log::info!("router {}: dropping invalid ipv4 packet", addr);
        return;
    };
    let dest = packet.get_destination();
    if dest == addr {
        log::info!("router {}: dropping packet addressed to me", addr);
        return;
    }
    for (_, tx, routes) in conns {
        for route in routes {
            if route.dest().contains(dest) || dest.is_broadcast() || dest.is_multicast() {
                log::debug!("router {}: routing packet on route {:?}", addr, route);
                let _ = tx.unbounded_send(bytes);
                return;
            }
        }
    }
    log::info!("router {}: dropping unroutable packet to {}", addr, dest);
}
