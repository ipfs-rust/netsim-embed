use crate::port_allocator::{PortAllocator, SequentialPortAllocator};
use std::collections::hash_map::{Entry, HashMap};
use std::net::SocketAddrV4;

#[derive(Debug)]
pub struct PortMap {
    map_out: HashMap<SocketAddrV4, u16>,
    map_in: HashMap<u16, SocketAddrV4>,
    allowed_endpoints: Option<HashMap<u16, SocketAddrV4>>,
    symmetric_map: Option<SymmetricMap>,
    port_allocator: Box<dyn PortAllocator>,
}

#[allow(clippy::derivable_impls)]
impl Default for PortMap {
    fn default() -> Self {
        Self {
            map_out: Default::default(),
            map_in: Default::default(),
            allowed_endpoints: Default::default(),
            symmetric_map: Default::default(),
            port_allocator: Box::<SequentialPortAllocator>::default(),
        }
    }
}

#[derive(Debug, Default)]
pub struct SymmetricMap {
    map_out: HashMap<(SocketAddrV4, SocketAddrV4), u16>,
    map_in: HashMap<u16, (SocketAddrV4, SocketAddrV4)>,
}

impl PortMap {
    pub fn forward_port(&mut self, port: u16, local_addr: SocketAddrV4) {
        self.map_out.insert(local_addr, port);
        self.map_in.insert(port, local_addr);
    }

    pub fn set_port_allocator<T: PortAllocator + 'static>(&mut self, port_allocator: T) {
        self.port_allocator = Box::new(port_allocator);
    }

    pub fn set_restrict_endpoints(&mut self, restrict_endpoints: bool) {
        if restrict_endpoints {
            self.allowed_endpoints = Some(Default::default());
        } else {
            self.allowed_endpoints = None;
        }
    }

    pub fn set_symmetric(&mut self, symmetric: bool) {
        if symmetric {
            self.symmetric_map = Some(Default::default());
        } else {
            self.symmetric_map = None;
        }
    }

    pub fn get_inbound_addr(&self, remote_addr: SocketAddrV4, port: u16) -> Option<SocketAddrV4> {
        if let Some(ref allowed_endpoints) = self.allowed_endpoints {
            if !allowed_endpoints
                .get(&port)
                .map(|allowed| *allowed == remote_addr)
                .unwrap_or(false)
            {
                log::trace!(
                    "NAT dropping packet from restricted address {}. allowed endpoints: {:?}",
                    remote_addr,
                    allowed_endpoints
                );
                return None;
            }
        }
        if let Some(addr) = self.map_in.get(&port) {
            return Some(*addr);
        }
        if let Some(ref symmetric_map) = self.symmetric_map {
            if let Some(&(addr, allowed_remote_addr)) = symmetric_map.map_in.get(&port) {
                if allowed_remote_addr == remote_addr {
                    return Some(addr);
                }
            }
        }
        None
    }

    pub fn map_port(&mut self, remote_addr: SocketAddrV4, source_addr: SocketAddrV4) -> u16 {
        let port = match self.map_out.entry(source_addr) {
            Entry::Occupied(oe) => *oe.get(),
            Entry::Vacant(ve) => {
                if let Some(ref mut symmetric_map) = self.symmetric_map {
                    match symmetric_map.map_out.entry((source_addr, remote_addr)) {
                        Entry::Occupied(oe) => *oe.get(),
                        Entry::Vacant(ve) => {
                            let port = loop {
                                let port = self.port_allocator.next_port(source_addr);
                                if self.map_in.contains_key(&port) {
                                    continue;
                                }
                                if symmetric_map.map_in.contains_key(&port) {
                                    continue;
                                }
                                break port;
                            };

                            ve.insert(port);
                            symmetric_map
                                .map_in
                                .insert(port, (source_addr, remote_addr));
                            port
                        }
                    }
                } else {
                    let port = loop {
                        let port = self.port_allocator.next_port(source_addr);
                        if self.map_in.contains_key(&port) {
                            continue;
                        }
                        break port;
                    };

                    ve.insert(port);
                    self.map_in.insert(port, source_addr);
                    port
                }
            }
        };
        if let Some(ref mut allowed_endpoints) = self.allowed_endpoints {
            allowed_endpoints.insert(port, remote_addr);
        }
        port
    }
}
