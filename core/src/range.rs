use crate::addr::{Ipv4AddrClass, Ipv4AddrExt};
use std::net::Ipv4Addr;
use std::str::FromStr;
use thiserror::Error;

/// A range of IPv4 addresses with a common prefix
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
    ///
    /// # Example
    ///
    /// Create the subnet 192.168.0.0/24 with `Ipv4Range::new("192.168.0.0".parse().unwrap(), 24)`
    pub fn new(addr: Ipv4Addr, bits: u8) -> Self {
        let mask = !((!0u32).checked_shr(u32::from(bits)).unwrap_or(0));
        Ipv4Range {
            addr: Ipv4Addr::from(u32::from(addr) & mask),
            bits,
        }
    }

    /// Return the entire IPv4 range, eg. 0.0.0.0/0
    pub fn global() -> Self {
        Ipv4Range {
            addr: Ipv4Addr::new(0, 0, 0, 0),
            bits: 0,
        }
    }

    /// Returns the local network subnet 10.0.0.0/8
    pub fn local_subnet_10() -> Self {
        Ipv4Range {
            addr: Ipv4Addr::new(10, 0, 0, 0),
            bits: 8,
        }
    }

    /// Returns a local network subnet 172.(16 | x).0.0/16 where x is a 4-bit number given by
    /// `block`
    ///
    /// # Panics
    ///
    /// If `block & 0xf0 != 0`
    pub fn local_subnet_172(block: u8) -> Self {
        assert!(block < 16);
        Ipv4Range {
            addr: Ipv4Addr::new(172, 16 | block, 0, 0),
            bits: 16,
        }
    }

    /// Returns the local subnet 192.168.x.0/24 where x is given by `block`.
    pub fn local_subnet_192(block: u8) -> Self {
        Ipv4Range {
            addr: Ipv4Addr::new(192, 168, block, 0),
            bits: 24,
        }
    }

    /// Returns a random local network subnet from one of the ranges 10.0.0.0, 172.16.0.0 or
    /// 192.168.0.0
    pub fn random_local_subnet() -> Self {
        match rand::random::<u8>() % 3 {
            0 => Ipv4Range::local_subnet_10(),
            1 => Ipv4Range::local_subnet_172(rand::random::<u8>() & 0x0f),
            2 => Ipv4Range::local_subnet_192(rand::random()),
            _ => unreachable!(),
        }
    }

    /// Get the netmask as an IP address
    pub fn netmask(&self) -> Ipv4Addr {
        Ipv4Addr::from(!((!0u32).checked_shr(u32::from(self.bits)).unwrap_or(0)))
    }

    /// Get the number of netmask prefix bits
    pub fn netmask_prefix_length(&self) -> u8 {
        self.bits
    }

    /// Get the base address of the range, ie. the lowest IP address which is part of the range.
    pub fn base_addr(&self) -> Ipv4Addr {
        self.addr
    }

    /// Get a default IP address for the range's gateway. This is one higher than the base address
    /// of the range. eg. for 10.0.0.0/8, the default address for the gateway will be 10.0.0.1
    pub fn gateway_addr(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.addr) | 1)
    }

    /// Get the broadcast address, ie. the highest IP address which is part of the range.
    pub fn broadcast_addr(&self) -> Ipv4Addr {
        Ipv4Addr::from(!(!0 >> self.bits) | u32::from(self.addr))
    }

    /// Get a random IP address from the range which is not the base address or the default
    /// for the gateway address.
    pub fn random_client_addr(&self) -> Ipv4Addr {
        let mask = !0 >> self.bits;
        assert!(mask > 1);
        let class = if self.bits == 0 {
            Ipv4AddrClass::Global
        } else {
            self.addr.class()
        };

        loop {
            let x = rand::random::<u32>() & mask;
            if x < 2 {
                continue;
            }
            let addr = Ipv4Addr::from(u32::from(self.addr) | x);
            if class != addr.class() {
                continue;
            }
            return addr;
        }
    }

    /// Generate an IP address for a device.
    pub fn address_for(&self, device: u32) -> Ipv4Addr {
        let mask = !0 >> self.bits;
        assert!(mask > 1);
        let addr = Ipv4Addr::from(u32::from(self.addr) | ((device & mask) + 2));
        assert_ne!(addr, self.broadcast_addr());
        addr
    }

    /// Check whether this range contains the given IP address
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        let base_addr = u32::from(self.addr);
        let test_addr = u32::from(ip);
        (base_addr ^ test_addr).leading_zeros() >= u32::from(self.bits)
    }

    /// Split a range into `num` sub-ranges
    ///
    /// # Panics
    ///
    /// If the range is too small to be split up that much.
    pub fn split(self, num: u32) -> Vec<Self> {
        let mut ret = Vec::with_capacity(num as usize);
        let mut n = 0u32;
        let class = if self.bits == 0 {
            Ipv4AddrClass::Global
        } else {
            self.addr.class()
        };
        loop {
            let mut n_reversed = 0;
            for i in 0..32 {
                if n & (1 << i) != 0 {
                    n_reversed |= 0x8000_0000u32 >> i;
                }
            }
            let base_addr = u32::from(self.addr);
            let ip = base_addr | (n_reversed >> self.bits);
            let ip = Ipv4Addr::from(ip);
            if class != ip.class() {
                n += 1;
                continue;
            }
            ret.push(Ipv4Range { addr: ip, bits: 0 });
            if ret.len() == num as usize {
                break;
            }
            n += 1;
        }
        let extra_bits = (32 - n.leading_zeros()) as u8;
        let bits = self.bits + extra_bits;
        for range in &mut ret {
            range.bits = bits;
        }
        ret
    }
}

/// Errors returned by `SubnetV*::from_str`
#[derive(Debug, Error)]
pub enum IpRangeParseError {
    /// Missing '/' delimiter
    #[error("missing '/' delimiter")]
    MissingDelimiter,
    /// More than one '/' delimiter
    #[error("more than one '/' delimiter")]
    ExtraDelimiter,
    /// error parsing IP address
    #[error("error parsing IP address: {0}")]
    ParseAddr(std::net::AddrParseError),
    /// error parsing netmask prefix length
    #[error("error parsing netmask prefix length: {0}")]
    ParseNetmaskPrefixLength(std::num::ParseIntError),
}

impl FromStr for Ipv4Range {
    type Err = IpRangeParseError;

    fn from_str(s: &str) -> Result<Ipv4Range, IpRangeParseError> {
        let mut split = s.split('/');
        let addr = split.next().unwrap();
        let bits = match split.next() {
            Some(bits) => bits,
            None => return Err(IpRangeParseError::MissingDelimiter),
        };
        if split.next().is_some() {
            return Err(IpRangeParseError::ExtraDelimiter);
        }
        let addr = match Ipv4Addr::from_str(addr) {
            Ok(addr) => addr,
            Err(e) => return Err(IpRangeParseError::ParseAddr(e)),
        };
        let bits = match u8::from_str(bits) {
            Ok(bits) => bits,
            Err(e) => return Err(IpRangeParseError::ParseNetmaskPrefixLength(e)),
        };
        Ok(Ipv4Range::new(addr, bits))
    }
}

impl From<Ipv4Addr> for Ipv4Range {
    fn from(addr: Ipv4Addr) -> Self {
        Self::new(addr, 32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_creates_address_range() {
        let addrs = Ipv4Range::new("1.2.3.0".parse().unwrap(), 24);

        assert!(addrs.contains("1.2.3.5".parse().unwrap()));
        assert!(addrs.contains("1.2.3.255".parse().unwrap()));
        assert!(!addrs.contains("1.2.4.5".parse().unwrap()));
    }
}
