use std::collections::hash_map::{Entry, HashMap};
use std::net::SocketAddrV4;

#[derive(Debug)]
pub enum PortAllocator {
    Sequential {
        next_original_port: u16,
        next_for_local_endpoint: HashMap<SocketAddrV4, u16>,
    },
    Random,
}

impl Default for PortAllocator {
    fn default() -> PortAllocator {
        PortAllocator::Sequential {
            next_original_port: 49152,
            next_for_local_endpoint: HashMap::new(),
        }
    }
}

impl PortAllocator {
    pub fn next_port(&mut self, local_endpoint: SocketAddrV4) -> u16 {
        match *self {
            PortAllocator::Sequential {
                ref mut next_original_port,
                ref mut next_for_local_endpoint,
            } => match next_for_local_endpoint.entry(local_endpoint) {
                Entry::Occupied(mut oe) => {
                    let port = *oe.get();
                    *oe.get_mut() = oe.get().checked_add(1).unwrap_or(49152);
                    port
                }
                Entry::Vacant(ve) => {
                    let port = *next_original_port;
                    *next_original_port = next_original_port.wrapping_add(16);
                    if *next_original_port < 49152 {
                        *next_original_port += 49153
                    };
                    ve.insert(port);
                    port
                }
            },
            PortAllocator::Random => loop {
                let port = rand::random();
                if port >= 1000 {
                    break port;
                }
            },
        }
    }
}
