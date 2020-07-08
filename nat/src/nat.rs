use crate::packet::{Packet, Protocol};
use crate::port_map::PortMap;
use futures::future::Future;
use smol_netsim_core::{Ipv4Range, Plug};
use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::pin::Pin;
use std::task::{Context, Poll};

/// An Ipv4 NAT.
#[derive(Debug)]
pub struct Ipv4Nat {
    pub(crate) private_plug: Plug,
    pub(crate) public_plug: Plug,
    pub(crate) public_ip: Ipv4Addr,
    pub(crate) subnet: Ipv4Range,
    pub(crate) hair_pinning: bool,
    pub(crate) udp_map: PortMap,
    pub(crate) tcp_map: PortMap,
    pub(crate) blacklist_unrecognized_addrs: bool,
    pub(crate) blacklisted_addrs: HashSet<SocketAddrV4>,
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
                        log::info!(
                            "nat {}: rewrote packet destination address: {} => {}",
                            self.public_ip,
                            dest_addr,
                            private_dest_addr,
                        );
                        packet.set_checksum();
                        let _ = self.private_plug.unbounded_send(bytes);
                    } else {
                        packet.set_source(external_source_addr);
                        log::info!(
                            "nat {}: rewrote packet source address: {} => {}",
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
                    if let Some(private_dest_addr) =
                        self.udp_map.get_inbound_addr(source_addr, dest_addr.port())
                    {
                        packet.set_destination(private_dest_addr);
                        log::info!(
                            "nat {}: rewrote destination of inbound packet {} => {}.",
                            self.public_ip,
                            dest_addr,
                            private_dest_addr,
                        );
                        packet.set_checksum();
                        let _ = self.private_plug.unbounded_send(bytes);
                    } else {
                        if self.blacklist_unrecognized_addrs {
                            log::info!(
                                "nat {}: blacklisting unknown address {}.",
                                self.public_ip,
                                source_addr
                            );
                            self.blacklisted_addrs.insert(source_addr);
                        }
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
