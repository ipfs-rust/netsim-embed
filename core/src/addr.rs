use std::net::Ipv4Addr;

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub enum Ipv4AddrClass {
    Unspecified,
    CurrentNetwork,
    Private,
    CarrierNat,
    Loopback,
    LinkLocal,
    ProtocolAssignments,
    Testnet,
    Ipv6Relay,
    BenchmarkTests,
    Multicast,
    Reserved,
    Broadcast,
    Global,
}

/// Extension methods for IPv4 addresses
pub trait Ipv4AddrExt {
    /// Get a random, global IPv4 address.
    fn random_global() -> Ipv4Addr;
    /// Returns `true` if this is a global IPv4 address
    fn is_global(&self) -> bool;
    /// Returns `true` if this is a reserved IPv4 address.
    fn is_reserved_addr(&self) -> bool;
    /// Clasify the address.
    fn class(&self) -> Ipv4AddrClass;
    /// Create an `Ipv4Addr` representing a netmask
    fn from_netmask_bits(bits: u8) -> Ipv4Addr;
}

impl Ipv4AddrExt for Ipv4Addr {
    fn random_global() -> Ipv4Addr {
        loop {
            let x: u32 = rand::random();
            let ip = Ipv4Addr::from(x);
            if Ipv4AddrExt::is_global(&ip) {
                return ip;
            }
        }
    }

    fn is_global(&self) -> bool {
        !(self.is_loopback()
            || self.is_private()
            || self.is_link_local()
            || self.is_multicast()
            || self.is_broadcast()
            || self.is_documentation()
            || self.is_reserved_addr())
    }

    fn is_reserved_addr(&self) -> bool {
        u32::from(*self) & 0xf000_0000 == 0xf000_0000
    }

    fn class(&self) -> Ipv4AddrClass {
        let ip = u32::from(*self);
        match ip {
            0x00000000 => Ipv4AddrClass::Unspecified,
            p if (0x00000000..0x01000000).contains(&p) => Ipv4AddrClass::CurrentNetwork,
            p if (0x0a000000..0x0b000000).contains(&p) => Ipv4AddrClass::Private,
            p if (0x64400000..0x64800000).contains(&p) => Ipv4AddrClass::CarrierNat,
            p if (0x7f000000..0x80000000).contains(&p) => Ipv4AddrClass::Loopback,
            p if (0xa9fe0000..0xa9ff0000).contains(&p) => Ipv4AddrClass::LinkLocal,
            p if (0xac100000..0xac200000).contains(&p) => Ipv4AddrClass::Private,
            p if (0xc0000000..0xc0000100).contains(&p) => Ipv4AddrClass::ProtocolAssignments,
            p if (0xc0000200..0xc0000300).contains(&p) => Ipv4AddrClass::Testnet,
            p if (0xc0586300..0xc0586400).contains(&p) => Ipv4AddrClass::Ipv6Relay,
            p if (0xc0a80000..0xc0a90000).contains(&p) => Ipv4AddrClass::Private,
            p if (0xc6120000..0xc6140000).contains(&p) => Ipv4AddrClass::BenchmarkTests,
            p if (0xc6336400..0xc6336500).contains(&p) => Ipv4AddrClass::Testnet,
            p if (0xcb007100..0xcb007200).contains(&p) => Ipv4AddrClass::Testnet,
            p if (0xe0000000..0xf0000000).contains(&p) => Ipv4AddrClass::Multicast,
            p if (0xf0000000..0xffffffff).contains(&p) => Ipv4AddrClass::Reserved,
            0xffffffff => Ipv4AddrClass::Broadcast,
            _ => Ipv4AddrClass::Global,
        }
    }

    fn from_netmask_bits(bits: u8) -> Ipv4Addr {
        Ipv4Addr::from(!((!0u32) >> bits))
    }
}
