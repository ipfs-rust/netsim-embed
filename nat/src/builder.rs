use crate::nat::Ipv4Nat;
use crate::port_allocator::PortAllocator;
use crate::port_map::{PortMap, SymmetricMap};
use netsim_embed_core::{Ipv4Range, Plug};
use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddrV4};

/// A builder for `Ipv4Nat`
#[derive(Default)]
pub struct Ipv4NatBuilder {
    subnet: Option<Ipv4Range>,
    hair_pinning: bool,
    udp_map: PortMap,
    tcp_map: PortMap,
    blacklist_unrecognized_addrs: bool,
}

impl Ipv4NatBuilder {
    /// Start building an Ipv4 NAT
    pub fn new() -> Ipv4NatBuilder {
        Self::default()
    }

    /// Set the subnet used on the local side of the NAT. If left unset, a random subnet will be
    /// chosen.
    pub fn subnet(mut self, subnet: Ipv4Range) -> Ipv4NatBuilder {
        self.subnet = Some(subnet);
        self
    }

    /// Enable/disable hair-pinning.
    pub fn hair_pinning(mut self, hair_pinning: bool) -> Ipv4NatBuilder {
        self.hair_pinning = hair_pinning;
        self
    }

    /// Manually forward a UDP port.
    pub fn forward_udp_port(mut self, port: u16, local_addr: SocketAddrV4) -> Ipv4NatBuilder {
        self.udp_map.forward_port(port, local_addr);
        self
    }

    /// Manually forward a TCP port.
    pub fn forward_tcp_port(mut self, port: u16, local_addr: SocketAddrV4) -> Ipv4NatBuilder {
        self.tcp_map.forward_port(port, local_addr);
        self
    }

    /// Causes the NAT to permanently block all traffic from an address A if it receives traffic
    /// from A directed at an endpoint for which is doesn't have a mapping.
    pub fn blacklist_unrecognized_addrs(mut self) -> Ipv4NatBuilder {
        self.blacklist_unrecognized_addrs = true;
        self
    }

    /// Only allow incoming traffic on a port from remote addresses that we have already sent
    /// data to from that port. Makes this a port-restricted NAT.
    pub fn restrict_endpoints(mut self) -> Ipv4NatBuilder {
        self.tcp_map.allowed_endpoints = Some(HashMap::new());
        self.udp_map.allowed_endpoints = Some(HashMap::new());
        self
    }

    /// Use random, rather than sequential (the default) port allocation.
    pub fn randomize_port_allocation(mut self) -> Ipv4NatBuilder {
        self.tcp_map.port_allocator = PortAllocator::Random;
        self.udp_map.port_allocator = PortAllocator::Random;
        self
    }

    /// Makes this NAT a symmetric NAT, meaning packets sent to different remote addresses from the
    /// same internal address will appear to originate from different external ports.
    pub fn symmetric(mut self) -> Ipv4NatBuilder {
        self.tcp_map.symmetric_map = Some(SymmetricMap::default());
        self.udp_map.symmetric_map = Some(SymmetricMap::default());
        self
    }

    /// Build the NAT
    pub fn build(
        self,
        public_plug: Plug,
        private_plug: Plug,
        public_ip: Ipv4Addr,
        subnet: Ipv4Range,
    ) -> Ipv4Nat {
        Ipv4Nat {
            private_plug,
            public_plug,
            public_ip,
            subnet,
            hair_pinning: self.hair_pinning,
            udp_map: self.udp_map,
            tcp_map: self.tcp_map,
            blacklist_unrecognized_addrs: false,
            blacklisted_addrs: HashSet::new(),
        }
    }
}
