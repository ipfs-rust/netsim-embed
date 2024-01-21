use futures::{
    channel::{mpsc, oneshot},
    future::{poll_fn, FutureExt},
    stream::{FuturesUnordered, StreamExt},
};
use libpacket::ipv4::Ipv4Packet;
use netsim_embed_core::{Ipv4Route, Plug};
use std::{
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::Poll,
};

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
    counters: Arc<Counters>,
}

pub type Filter = Box<dyn Fn(&[u8]) -> bool + Send + Sync + 'static>;

#[derive(Default)]
struct Counters {
    filter: Mutex<Option<Filter>>,
    forwarded: AtomicUsize,
    invalid: AtomicUsize,
    disabled: AtomicUsize,
    unroutable: AtomicUsize,
}

impl std::fmt::Debug for Counters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Counters")
            .field("forwarded", &self.forwarded)
            .field("invalid", &self.invalid)
            .field("disabled", &self.disabled)
            .field("unroutable", &self.unroutable)
            .finish()
    }
}

impl Ipv4Router {
    pub fn new(addr: Ipv4Addr) -> Self {
        let (tx, rx) = mpsc::unbounded();
        let counters = Arc::new(Counters::default());
        router(addr, Arc::clone(&counters), rx);
        Self {
            addr,
            ctrl: tx,
            counters,
        }
    }

    pub fn forwarded(&self) -> usize {
        self.counters.forwarded.load(Ordering::Relaxed)
    }

    pub fn invalid(&self) -> usize {
        self.counters.invalid.load(Ordering::Relaxed)
    }

    pub fn disabled(&self) -> usize {
        self.counters.disabled.load(Ordering::Relaxed)
    }

    pub fn unroutable(&self) -> usize {
        self.counters.unroutable.load(Ordering::Relaxed)
    }

    pub fn set_filter(&self, filter: Option<Filter>) {
        *self.counters.filter.lock().unwrap() = filter;
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

fn router(addr: Ipv4Addr, counters: Arc<Counters>, mut ctrl: mpsc::UnboundedReceiver<RouterCtrl>) {
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
                    (_, Some(packet)) => forward_packet(addr, &counters, &mut conns, packet),
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
    counters: &Counters,
    conns: &mut [(usize, Plug, Vec<Ipv4Route>, bool)],
    bytes: Vec<u8>,
) {
    let count = counters.filter.lock().unwrap().iter().all(|f| f(&bytes));
    let packet = if let Some(packet) = Ipv4Packet::new(&bytes) {
        packet
    } else {
        if count {
            counters.invalid.fetch_add(1, Ordering::Relaxed);
        }
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
                    if count {
                        counters.disabled.fetch_add(1, Ordering::Relaxed);
                    }
                    log::trace!("router {}: route {:?} disabled", addr, route);
                } else {
                    if count {
                        counters.forwarded.fetch_add(1, Ordering::Relaxed);
                    }
                    log::trace!("router {}: routing packet on route {:?}", addr, route,);
                    tx.unbounded_send(bytes.clone());
                    forwarded = true;
                }
            }
        }
    }
    if !forwarded {
        let src = packet.get_source();
        if count {
            counters.unroutable.fetch_add(1, Ordering::Relaxed);
        }
        log::debug!(
            "router {}: dropping unroutable packet from {} to {}",
            addr,
            src,
            dest
        );
    }
}
