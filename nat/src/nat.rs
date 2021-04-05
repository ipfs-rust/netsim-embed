use crate::port_allocator::PortAllocator;
use crate::port_map::PortMap;
use futures::future::Future;
use netsim_embed_core::{Ipv4Range, Packet, Plug, Protocol};
use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::pin::Pin;
use std::task::{Context, Poll};

/// An Ipv4 NAT.
#[derive(Debug)]
pub struct Ipv4Nat {
    private_plug: Plug,
    public_plug: Plug,
    public_ip: Ipv4Addr,
    subnet: Ipv4Range,
    hair_pinning: bool,
    udp_map: PortMap,
    tcp_map: PortMap,
    blacklist_unrecognized_addrs: bool,
    blacklisted_addrs: HashSet<SocketAddrV4>,
}

impl Ipv4Nat {
    pub fn new(
        public_plug: Plug,
        private_plug: Plug,
        public_ip: Ipv4Addr,
        subnet: Ipv4Range,
    ) -> Self {
        Self {
            private_plug,
            public_plug,
            public_ip,
            subnet,
            hair_pinning: false,
            udp_map: Default::default(),
            tcp_map: Default::default(),
            blacklist_unrecognized_addrs: false,
            blacklisted_addrs: Default::default(),
        }
    }

    /// Set the port allocator.
    pub fn set_port_allocator<T: Clone + PortAllocator + 'static>(&mut self, port_allocator: T) {
        self.udp_map.set_port_allocator(port_allocator.clone());
        self.tcp_map.set_port_allocator(port_allocator);
    }

    /// Enable/disable hair-pinning.
    pub fn set_hair_pinning(&mut self, hair_pinning: bool) {
        self.hair_pinning = hair_pinning;
    }

    /// Manually forward a port.
    pub fn forward_port(&mut self, port: u16, local_addr: SocketAddrV4, protocol: Protocol) {
        match protocol {
            Protocol::Udp => self.udp_map.forward_port(port, local_addr),
            Protocol::Tcp => self.tcp_map.forward_port(port, local_addr),
        }
    }

    /// Causes the NAT to permanently block all traffic from an address A if it recieves
    /// traffic from A directed at an endpoint for which it doesn't have a mapping.
    pub fn set_blacklist_unrecognized_addrs(&mut self, blacklist_unrecognized_addrs: bool) {
        self.blacklist_unrecognized_addrs = blacklist_unrecognized_addrs;
    }

    /// Only allow incoming traffic on a port from remote addresses that we have already
    /// sent data to from that port. Makes this a port-restricted NAT.
    pub fn set_restrict_endpoints(&mut self, restrict_endpoints: bool) {
        self.udp_map.set_restrict_endpoints(restrict_endpoints);
        self.tcp_map.set_restrict_endpoints(restrict_endpoints);
    }

    /// Makes this NAT a symmetric NAT, meaning packets sent to different remote addresses from
    /// the same internal address will appear to originate from different external ports.
    pub fn set_symmetric(&mut self, symmetric: bool) {
        self.udp_map.set_symmetric(symmetric);
        self.tcp_map.set_symmetric(symmetric);
    }
}

impl Ipv4Nat {
    fn process_outgoing(&mut self, cx: &mut Context) -> bool {
        loop {
            match self.private_plug.poll_incoming(cx) {
                Poll::Pending => return false,
                Poll::Ready(None) => return true,
                Poll::Ready(Some(mut bytes)) => {
                    let mut packet = if let Some(packet) = Packet::new(&mut bytes) {
                        packet
                    } else {
                        log::info!("nat {}: dropping invalid outbound packet", self.public_ip);
                        continue;
                    };
                    let source_addr = packet.get_source();
                    let dest_addr = packet.get_destination();

                    if !self.subnet.contains(*source_addr.ip()) {
                        log::info!(
                            "nat {}: dropping outbound packet which does not originate from our subnet.",
                            self.public_ip,
                        );
                        continue;
                    }

                    let next_ttl = match packet.get_ttl().checked_sub(1) {
                        Some(ttl) => ttl,
                        None => {
                            log::info!(
                                "nat {} dropping outbound packet with ttl zero.",
                                self.public_ip,
                            );
                            continue;
                        }
                    };
                    packet.set_ttl(next_ttl);

                    let map = match packet.protocol() {
                        Protocol::Udp => &mut self.udp_map,
                        Protocol::Tcp => &mut self.tcp_map,
                    };

                    let external_source_addr =
                        SocketAddrV4::new(self.public_ip, map.map_port(dest_addr, source_addr));

                    if self.hair_pinning && dest_addr.ip() == &self.public_ip {
                        let private_dest_addr = if let Some(addr) =
                            map.get_inbound_addr(external_source_addr, dest_addr.port())
                        {
                            addr
                        } else {
                            continue;
                        };
                        packet.set_destination(private_dest_addr);
                        log::trace!(
                            "nat {}: rewrote outbound packet destination address: {} => {}",
                            self.public_ip,
                            dest_addr,
                            private_dest_addr,
                        );
                        packet.set_checksum();
                        let _ = self.private_plug.unbounded_send(bytes);
                    } else {
                        packet.set_source(external_source_addr);
                        log::trace!(
                            "nat {}: rewrote outbound packet source address: {} => {}",
                            self.public_ip,
                            source_addr,
                            external_source_addr,
                        );
                        packet.set_checksum();
                        let _ = self.public_plug.unbounded_send(bytes);
                    }
                }
            }
        }
    }

    fn process_incoming(&mut self, cx: &mut Context) -> bool {
        loop {
            match self.public_plug.poll_incoming(cx) {
                Poll::Pending => return false,
                Poll::Ready(None) => return true,
                Poll::Ready(Some(mut bytes)) => {
                    let mut packet = if let Some(packet) = Packet::new(&mut bytes) {
                        packet
                    } else {
                        log::info!("nat {}: dropping invalid inbound packet.", self.public_ip);
                        continue;
                    };
                    let source_addr = packet.get_source();
                    let dest_addr = packet.get_destination();

                    if dest_addr.ip() != &self.public_ip {
                        log::info!(
                            "nat {} dropping inbound packet not directed at our public ip.",
                            self.public_ip,
                        );
                        continue;
                    }

                    let next_ttl = match packet.get_ttl().checked_sub(1) {
                        Some(ttl) => ttl,
                        None => {
                            log::info!(
                                "nat {} dropping inbound packet with ttl zero.",
                                self.public_ip,
                            );
                            continue;
                        }
                    };
                    packet.set_ttl(next_ttl);

                    if self.blacklisted_addrs.contains(&source_addr) {
                        log::info!(
                            "nat {} dropped packet from blacklisted addr {}.",
                            self.public_ip,
                            source_addr
                        );
                        continue;
                    }

                    let map = match packet.protocol() {
                        Protocol::Udp => &mut self.udp_map,
                        Protocol::Tcp => &mut self.tcp_map,
                    };

                    if let Some(private_dest_addr) =
                        map.get_inbound_addr(source_addr, dest_addr.port())
                    {
                        packet.set_destination(private_dest_addr);
                        log::trace!(
                            "nat {}: rewrote inbound packet destination address: {} => {}.",
                            self.public_ip,
                            dest_addr,
                            private_dest_addr,
                        );
                        packet.set_checksum();
                        let _ = self.private_plug.unbounded_send(bytes);
                    } else if self.blacklist_unrecognized_addrs {
                        log::info!(
                            "nat {}: blacklisting unknown address {}.",
                            self.public_ip,
                            source_addr,
                        );
                        self.blacklisted_addrs.insert(source_addr);
                    } else {
                        log::info!(
                            "nat {}: dropping packet to unknown inbound destination {}.",
                            self.public_ip,
                            dest_addr,
                        );
                        log::info!("{:?}", map);
                    }
                }
            }
        }
    }
}

impl Future for Ipv4Nat {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let private_unplugged = self.process_outgoing(cx);
        let public_unplugged = self.process_incoming(cx);

        if private_unplugged && public_unplugged {
            return Poll::Ready(());
        }

        Poll::Pending
    }
}
