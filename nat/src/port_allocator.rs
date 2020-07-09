use std::collections::hash_map::{Entry, HashMap};
use std::net::SocketAddrV4;

pub trait PortAllocator: std::fmt::Debug + Send {
    fn next_port(&mut self, local_endpoint: SocketAddrV4) -> u16;
}

#[derive(Clone, Debug)]
pub struct SequentialPortAllocator {
    next_original_port: u16,
    next_for_local_endpoint: HashMap<SocketAddrV4, u16>,
}

impl Default for SequentialPortAllocator {
    fn default() -> Self {
        Self {
            next_original_port: 49152,
            next_for_local_endpoint: HashMap::new(),
        }
    }
}

impl PortAllocator for SequentialPortAllocator {
    fn next_port(&mut self, local_endpoint: SocketAddrV4) -> u16 {
        match self.next_for_local_endpoint.entry(local_endpoint) {
            Entry::Occupied(mut entry) => {
                let port = *entry.get();
                *entry.get_mut() = entry.get().checked_add(1).unwrap_or(49152);
                port
            }
            Entry::Vacant(entry) => {
                let port = self.next_original_port;
                self.next_original_port = self.next_original_port.wrapping_add(16);
                if self.next_original_port < 49152 {
                    self.next_original_port += 49153
                };
                entry.insert(port);
                port
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RandomPortAllocator;

impl PortAllocator for RandomPortAllocator {
    fn next_port(&mut self, _local_endpoint: SocketAddrV4) -> u16 {
        loop {
            let port = rand::random();
            if port >= 1000 {
                return port;
            }
        }
    }
}
