use pnet_packet::ip::IpNextHeaderProtocols;
use pnet_packet::ipv4::{Ipv4Packet, MutableIpv4Packet};
use pnet_packet::tcp::{MutableTcpPacket, TcpPacket};
use pnet_packet::udp::{MutableUdpPacket, UdpPacket};
use pnet_packet::{MutablePacket, Packet as _};
use std::net::SocketAddrV4;

#[derive(Clone, Copy, Debug)]
pub enum Protocol {
    Udp,
    Tcp,
}

#[derive(Debug)]
pub struct Packet<'a> {
    protocol: Protocol,
    bytes: &'a mut [u8],
}

impl<'a> Packet<'a> {
    pub fn new(bytes: &'a mut [u8]) -> Option<Self> {
        let packet = if let Some(packet) = Ipv4Packet::new(bytes) {
            packet
        } else {
            return None;
        };
        let protocol = match packet.get_next_level_protocol() {
            IpNextHeaderProtocols::Udp => {
                if UdpPacket::new(packet.payload()).is_none() {
                    return None;
                }
                Protocol::Udp
            }
            IpNextHeaderProtocols::Tcp => {
                if TcpPacket::new(packet.payload()).is_none() {
                    return None;
                }
                Protocol::Tcp
            }
            _ => return None,
        };
        Some(Self { protocol, bytes })
    }

    pub fn get_source(&self) -> SocketAddrV4 {
        let packet = Ipv4Packet::new(self.bytes).unwrap();
        let ip = packet.get_source();
        let port = match self.protocol {
            Protocol::Udp => UdpPacket::new(packet.payload()).unwrap().get_source(),
            Protocol::Tcp => TcpPacket::new(packet.payload()).unwrap().get_source(),
        };
        SocketAddrV4::new(ip, port.into())
    }

    pub fn get_destination(&self) -> SocketAddrV4 {
        let packet = Ipv4Packet::new(self.bytes).unwrap();
        let ip = packet.get_destination();
        let port = match self.protocol {
            Protocol::Udp => UdpPacket::new(packet.payload()).unwrap().get_destination(),
            Protocol::Tcp => TcpPacket::new(packet.payload()).unwrap().get_destination(),
        };
        SocketAddrV4::new(ip, port.into())
    }

    pub fn get_ttl(&self) -> u8 {
        Ipv4Packet::new(self.bytes).unwrap().get_ttl()
    }

    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    pub fn set_source(&mut self, addr: SocketAddrV4) {
        let mut packet = MutableIpv4Packet::new(self.bytes).unwrap();
        packet.set_source(*addr.ip());
        match self.protocol {
            Protocol::Udp => {
                let mut udp = MutableUdpPacket::new(packet.payload_mut()).unwrap();
                udp.set_source(addr.port().into());
            }
            Protocol::Tcp => {
                let mut tcp = MutableTcpPacket::new(packet.payload_mut()).unwrap();
                tcp.set_source(addr.port().into());
            }
        }
    }

    pub fn set_destination(&mut self, addr: SocketAddrV4) {
        let mut packet = MutableIpv4Packet::new(self.bytes).unwrap();
        packet.set_destination(*addr.ip());
        match self.protocol {
            Protocol::Udp => {
                let mut udp = MutableUdpPacket::new(packet.payload_mut()).unwrap();
                udp.set_destination(addr.port().into());
            }
            Protocol::Tcp => {
                let mut tcp = MutableTcpPacket::new(packet.payload_mut()).unwrap();
                tcp.set_destination(addr.port().into());
            }
        }
    }

    pub fn set_ttl(&mut self, ttl: u8) {
        MutableIpv4Packet::new(self.bytes).unwrap().set_ttl(ttl)
    }
}
