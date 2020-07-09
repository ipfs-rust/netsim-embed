use futures::channel::mpsc;
use futures::future::Future;
use futures::stream::Stream;
use netsim_embed_core::{Ipv4Route, Plug};
use pnet_packet::ipv4::Ipv4Packet;
use std::net::Ipv4Addr;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Debug)]
pub struct Ipv4Router {
    addr: Ipv4Addr,
    rxs: Vec<mpsc::UnboundedReceiver<Vec<u8>>>,
    txs: Vec<(mpsc::UnboundedSender<Vec<u8>>, Vec<Ipv4Route>)>,
}

impl Ipv4Router {
    pub fn new(addr: Ipv4Addr) -> Self {
        Self {
            addr,
            rxs: Default::default(),
            txs: Default::default(),
        }
    }

    pub fn add_connection(&mut self, plug: Plug, routes: Vec<Ipv4Route>) {
        let (tx, rx) = plug.split();
        self.rxs.push(rx);
        self.txs.push((tx, routes));
    }

    fn process_packet(&mut self, bytes: Vec<u8>) {
        let packet = if let Some(packet) = Ipv4Packet::new(&bytes) {
            packet
        } else {
            log::info!("router {}: dropping invalid ipv4 packet", self.addr);
            return;
        };
        let dest = packet.get_destination();
        if dest == self.addr {
            log::info!("router {}: dropping packet addressed to me", self.addr);
            return;
        }
        for (tx, routes) in &self.txs {
            for route in routes {
                if route.dest().contains(dest) {
                    log::info!("router {}: routing packet on route {:?}", self.addr, route);
                    let _ = tx.unbounded_send(bytes);
                    return;
                }
            }
            log::info!(
                "router {}: dropping unroutable packet to {}",
                self.addr,
                dest
            );
        }
    }
}

impl Future for Ipv4Router {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut i = 0;
        while i < self.rxs.len() {
            loop {
                let packet = match Pin::new(&mut self.rxs[i]).poll_next(cx) {
                    Poll::Pending => {
                        i += 1;
                        break;
                    }
                    Poll::Ready(None) => {
                        self.rxs.remove(i);
                        self.txs.remove(i);
                        break;
                    }
                    Poll::Ready(Some(packet)) => packet,
                };
                self.process_packet(packet)
            }
        }

        if self.rxs.is_empty() {
            return Poll::Ready(());
        }

        Poll::Pending
    }
}
