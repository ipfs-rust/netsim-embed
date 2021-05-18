use libpacket::ip::IpNextHeaderProtocols;
use libpacket::ipv4::{self, Ipv4Packet, MutableIpv4Packet};
use libpacket::tcp::{self, MutableTcpPacket, TcpPacket};
use libpacket::udp::{self, MutableUdpPacket, UdpPacket};
use libpacket::{MutablePacket, Packet as _};
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
        let packet = Ipv4Packet::new(bytes)?;
        let protocol = match packet.get_next_level_protocol() {
            IpNextHeaderProtocols::Udp => {
                UdpPacket::new(packet.payload())?;
                Protocol::Udp
            }
            IpNextHeaderProtocols::Tcp => {
                TcpPacket::new(packet.payload())?;
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
        SocketAddrV4::new(ip, port)
    }

    pub fn get_destination(&self) -> SocketAddrV4 {
        let packet = Ipv4Packet::new(self.bytes).unwrap();
        let ip = packet.get_destination();
        let port = match self.protocol {
            Protocol::Udp => UdpPacket::new(packet.payload()).unwrap().get_destination(),
            Protocol::Tcp => TcpPacket::new(packet.payload()).unwrap().get_destination(),
        };
        SocketAddrV4::new(ip, port)
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
                udp.set_source(addr.port());
            }
            Protocol::Tcp => {
                let mut tcp = MutableTcpPacket::new(packet.payload_mut()).unwrap();
                tcp.set_source(addr.port());
            }
        }
    }

    pub fn set_destination(&mut self, addr: SocketAddrV4) {
        let mut packet = MutableIpv4Packet::new(self.bytes).unwrap();
        packet.set_destination(*addr.ip());
        match self.protocol {
            Protocol::Udp => {
                let mut udp = MutableUdpPacket::new(packet.payload_mut()).unwrap();
                udp.set_destination(addr.port());
            }
            Protocol::Tcp => {
                let mut tcp = MutableTcpPacket::new(packet.payload_mut()).unwrap();
                tcp.set_destination(addr.port());
            }
        }
    }

    pub fn set_ttl(&mut self, ttl: u8) {
        MutableIpv4Packet::new(self.bytes).unwrap().set_ttl(ttl)
    }

    pub fn set_checksum(&mut self) {
        let mut packet = MutableIpv4Packet::new(self.bytes).unwrap();
        let source = packet.get_source();
        let dest = packet.get_destination();
        packet.set_checksum(ipv4::checksum(&packet.to_immutable()));
        match self.protocol {
            Protocol::Udp => {
                let mut udp = MutableUdpPacket::new(packet.payload_mut()).unwrap();
                udp.set_checksum(udp::ipv4_checksum(&udp.to_immutable(), &source, &dest));
            }
            Protocol::Tcp => {
                let mut tcp = MutableTcpPacket::new(packet.payload_mut()).unwrap();
                tcp.set_checksum(tcp::ipv4_checksum(&tcp.to_immutable(), &source, &dest));
            }
        }
    }
}
