use futures::channel::{mpsc, oneshot};
use futures::future::{poll_fn, FutureExt};
use futures::stream::{FuturesUnordered, StreamExt};
use libpacket::ipv4::Ipv4Packet;
use netsim_embed_core::{Ipv4Route, Plug};
use std::net::Ipv4Addr;
use std::task::Poll;

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
enum RouterCtrl {
    AddRoute(usize, Plug, Vec<Ipv4Route>),
    RemoveRoute(usize, oneshot::Sender<Option<Plug>>),
    EnableRoute(usize),
    DisableRoute(usize),
}

#[derive(Debug)]
pub struct Ipv4Router {
    #[allow(unused)]
    addr: Ipv4Addr,
    ctrl: mpsc::UnboundedSender<RouterCtrl>,
}

impl Ipv4Router {
    pub fn new(addr: Ipv4Addr) -> Self {
        let (tx, rx) = mpsc::unbounded();
        router(addr, rx);
        Self { addr, ctrl: tx }
    }

    pub fn add_connection(&self, id: usize, plug: Plug, routes: Vec<Ipv4Route>) {
        self.ctrl
            .unbounded_send(RouterCtrl::AddRoute(id, plug, routes))
            .ok();
    }

    pub async fn remove_connection(&self, id: usize) -> Option<Plug> {
        let (tx, rx) = oneshot::channel();
        self.ctrl
            .unbounded_send(RouterCtrl::RemoveRoute(id, tx))
            .unwrap();
        rx.await.unwrap()
    }

    pub fn enable_route(&self, id: usize) {
        self.ctrl
            .unbounded_send(RouterCtrl::EnableRoute(id))
            .unwrap();
    }

    pub fn disable_route(&self, id: usize) {
        self.ctrl
            .unbounded_send(RouterCtrl::DisableRoute(id))
            .unwrap();
    }
}

fn router(addr: Ipv4Addr, mut ctrl: mpsc::UnboundedReceiver<RouterCtrl>) {
    async_global_executor::spawn(async move {
        let mut conns = vec![];
        loop {
            futures::select! {
                ctrl = ctrl.next() => match ctrl {
                    Some(RouterCtrl::AddRoute(id, plug, routes)) => {
                        conns.push((id, plug, routes, true));
                    }
                    Some(RouterCtrl::RemoveRoute(id, ch)) => {
                        let plug = if let Some(idx) = conns.iter().position(|(id2, _, _, _)| *id2 == id) {
                            let (_, plug, _, _) = conns.swap_remove(idx);
                            Some(plug)
                        } else {
                            None
                        };
                        ch.send(plug).ok();
                    }
                    Some(RouterCtrl::EnableRoute(id)) => {
                        if let Some((_, _, _, en)) = conns.iter_mut().find(|(id2, _, _, _)| id == *id2) {
                            *en = true;
                        }
                    }
                    Some(RouterCtrl::DisableRoute(id)) => {
                        if let Some((_, _, _, en)) = conns.iter_mut().find(|(id2, _, _, _)| id == *id2) {
                            *en = false;
                        }
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

async fn incoming(conns: &mut [(usize, Plug, Vec<Ipv4Route>, bool)]) -> (usize, Option<Vec<u8>>) {
    let mut futures = conns
        .iter_mut()
        .enumerate()
        .filter(|(_, (_, _, _, en))| *en)
        .map(|(i, (_, plug, _, _))| async move { (i, plug.incoming().await) })
        .collect::<FuturesUnordered<_>>();
    if futures.is_empty() {
        poll_fn(|_| Poll::Pending).await
    } else {
        futures.next().await.unwrap()
    }
}

fn forward_packet(
    addr: Ipv4Addr,
    conns: &mut [(usize, Plug, Vec<Ipv4Route>, bool)],
    bytes: Vec<u8>,
) {
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
    let mut forwarded = false;
    for (_, tx, routes, en) in conns {
        for route in routes {
            if route.dest().contains(dest) || dest.is_broadcast() || dest.is_multicast() {
                if !*en {
                    log::trace!("router {}: route {:?} disabled", addr, route);
                } else {
                    log::trace!("router {}: routing packet on route {:?}", addr, route);
                    tx.unbounded_send(bytes.clone());
                    forwarded = true;
                }
            }
        }
    }
    if !forwarded {
        let src = packet.get_source();
        log::debug!(
            "router {}: dropping unroutable packet from {} to {}",
            addr,
            src,
            dest
        );
    }
}
