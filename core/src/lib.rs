use futures::channel::mpsc;
use std::net::Ipv4Addr;

#[derive(Clone, Copy)]
pub struct Ipv4Range {
    addr: Ipv4Addr,
    bits: u8,
}

impl std::fmt::Debug for Ipv4Range {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}/{}", self.addr, self.bits)
    }
}

impl Ipv4Range {
    /// Create an IPv4 range with the given base address and netmask prefix length.
    pub fn new(addr: Ipv4Addr, bits: u8) -> Self {
        let mask = !((!0u32).checked_shr(u32::from(bits)).unwrap_or(0));
        Self {
            addr: Ipv4Addr::from(u32::from(addr) & mask),
            bits,
        }
    }

    /// Returns the base address of the range, the lowest IP address which is part of the range.
    pub fn base_addr(&self) -> Ipv4Addr {
        self.addr
    }

    /// Return the default IP address for the range's gateway. This is one higher than the base
    /// address of the range. eg. for 10.0.0.0/8, the default address for the gateway will be
    /// 10.0.0.1
    pub fn gateway_addr(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.addr) | 1)
    }

    /// Returns the netmask prefix length.
    pub fn netmask_prefix_length(&self) -> u8 {
        self.bits
    }

    /// Returns the netmask of this range.
    pub fn netmask(&self) -> Ipv4Addr {
        Ipv4Addr::from(!((!0u32).checked_shr(u32::from(self.bits)).unwrap_or(0)))
    }

    /// Check whether a this range contains the given IP address.
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        let base_addr = u32::from(self.addr);
        let test_addr = u32::from(ip);
        (base_addr ^ test_addr).leading_zeros() >= u32::from(self.bits)
    }
}

impl From<Ipv4Addr> for Ipv4Range {
    fn from(addr: Ipv4Addr) -> Self {
        Self::new(addr, 32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ipv4Route {
    dest: Ipv4Range,
    gateway: Option<Ipv4Addr>,
}

impl Ipv4Route {
    /// Create a new route with the given destination and gateway.
    pub fn new(dest: Ipv4Range, gateway: Option<Ipv4Addr>) -> Self {
        Self {
            dest,
            gateway,
        }
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
    pub fn split(self) -> (mpsc::UnboundedSender<Vec<u8>>, mpsc::UnboundedReceiver<Vec<u8>>) {
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
