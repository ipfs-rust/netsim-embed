use futures::channel::mpsc;
use futures::stream::Stream;
use std::net::Ipv4Addr;
use std::pin::Pin;
use std::task::{Context, Poll};

mod addr;
mod range;

pub use range::Ipv4Range;

#[derive(Clone, Copy, Debug)]
pub struct Ipv4Route {
    dest: Ipv4Range,
    gateway: Option<Ipv4Addr>,
}

impl Ipv4Route {
    /// Create a new route with the given destination and gateway.
    pub fn new(dest: Ipv4Range, gateway: Option<Ipv4Addr>) -> Self {
        Self { dest, gateway }
    }

    /// Returns the destination IP range of the route.
    pub fn dest(&self) -> Ipv4Range {
        self.dest
    }

    /// Returns the route's gateway (if ayn).
    pub fn gateway(&self) -> Option<Ipv4Addr> {
        self.gateway
    }
}

impl From<Ipv4Range> for Ipv4Route {
    fn from(range: Ipv4Range) -> Self {
        Self::new(range, None)
    }
}

impl From<Ipv4Addr> for Ipv4Route {
    fn from(addr: Ipv4Addr) -> Self {
        Self::new(addr.into(), None)
    }
}

#[derive(Debug)]
pub struct Plug {
    tx: mpsc::UnboundedSender<Vec<u8>>,
    rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl Plug {
    pub fn poll_incoming(&mut self, cx: &mut Context) -> Poll<Option<Vec<u8>>> {
        Pin::new(&mut self.rx).poll_next(cx)
    }

    pub fn unbounded_send(&mut self, packet: Vec<u8>) {
        let _ = self.tx.unbounded_send(packet);
    }

    pub fn split(
        self,
    ) -> (
        mpsc::UnboundedSender<Vec<u8>>,
        mpsc::UnboundedReceiver<Vec<u8>>,
    ) {
        (self.tx, self.rx)
    }
}

pub fn wire() -> (Plug, Plug) {
    let (a_tx, b_rx) = mpsc::unbounded();
    let (b_tx, a_rx) = mpsc::unbounded();
    let a = Plug { tx: a_tx, rx: a_rx };
    let b = Plug { tx: b_tx, rx: b_rx };
    (a, b)
}
