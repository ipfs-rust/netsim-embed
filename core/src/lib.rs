use async_io::Timer;
use futures::channel::mpsc;
use futures::future::FutureExt;
use futures::stream::{Stream, StreamExt};
use std::collections::VecDeque;
use std::net::Ipv4Addr;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

mod addr;
mod packet;
mod range;

pub use packet::{Packet, Protocol};
pub use range::Ipv4Range;

#[derive(Clone, Copy, Debug)]
pub struct Ipv4Route {
    dest: Ipv4Range,
    gateway: Option<Ipv4Addr>,
}

impl Ipv4Route {
    /// Create a new route with the given destination and gateway.
    pub fn new(dest: Ipv4Range, gateway: Option<Ipv4Addr>) -> Self {
        Self { dest, gateway }
    }

    /// Returns the destination IP range of the route.
    pub fn dest(&self) -> Ipv4Range {
        self.dest
    }

    /// Returns the route's gateway (if ayn).
    pub fn gateway(&self) -> Option<Ipv4Addr> {
        self.gateway
    }
}

impl From<Ipv4Range> for Ipv4Route {
    fn from(range: Ipv4Range) -> Self {
        Self::new(range, None)
    }
}

impl From<Ipv4Addr> for Ipv4Route {
    fn from(addr: Ipv4Addr) -> Self {
        Self::new(addr.into(), None)
    }
}

#[derive(Debug)]
pub struct Plug {
    tx: mpsc::UnboundedSender<Vec<u8>>,
    rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl Plug {
    pub fn poll_incoming(&mut self, cx: &mut Context) -> Poll<Option<Vec<u8>>> {
        Pin::new(&mut self.rx).poll_next(cx)
    }

    pub async fn incoming(&mut self) -> Option<Vec<u8>> {
        self.rx.next().await
    }

    pub fn unbounded_send(&mut self, packet: Vec<u8>) {
        let _ = self.tx.unbounded_send(packet);
    }

    pub fn split(
        self,
    ) -> (
        mpsc::UnboundedSender<Vec<u8>>,
        mpsc::UnboundedReceiver<Vec<u8>>,
    ) {
        (self.tx, self.rx)
    }
}

pub fn wire() -> (Plug, Plug) {
    let (a_tx, b_rx) = mpsc::unbounded();
    let (b_tx, a_rx) = mpsc::unbounded();
    let a = Plug { tx: a_tx, rx: a_rx };
    let b = Plug { tx: b_tx, rx: b_rx };
    (a, b)
}

#[derive(Clone, Copy, Debug)]
pub struct DelayBuffer {
    delay: Duration,
    buffer_size: usize,
}

impl Default for DelayBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl DelayBuffer {
    pub fn new() -> Self {
        Self {
            delay: Duration::from_millis(0),
            buffer_size: usize::MAX,
        }
    }

    pub fn set_delay(&mut self, delay: Duration) {
        self.delay = delay;
    }

    pub fn set_buffer_size(&mut self, buffer_size: usize) {
        self.buffer_size = buffer_size;
    }

    pub fn spawn(self, mut b: Plug) -> Plug {
        #[allow(non_snake_case)]
        let DURATION_MAX: Duration = Duration::from_secs(10000);
        let (mut c, d) = wire();
        async_global_executor::spawn(async move {
            let mut b_tx_buffer_size = 0;
            let mut b_tx_buffer = VecDeque::new();
            let mut c_tx_buffer_size = 0;
            let mut c_tx_buffer = VecDeque::new();
            let mut idle = true;
            let mut timer = Timer::after(DURATION_MAX);
            loop {
                futures::select! {
                    packet = b.incoming().fuse() => {
                        if let Some(packet) = packet {
                            if c_tx_buffer_size + packet.len() < self.buffer_size {
                                c_tx_buffer_size += packet.len();
                                let time = Instant::now();
                                c_tx_buffer.push_back((packet, time + self.delay));
                                if idle {
                                    timer.set_after(self.delay);
                                    idle = false;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    packet = c.incoming().fuse() => {
                        if let Some(packet) = packet {
                            if b_tx_buffer_size + packet.len() < self.buffer_size {
                                b_tx_buffer_size += packet.len();
                                let time = Instant::now();
                                b_tx_buffer.push_back((packet, time + self.delay));
                                if idle {
                                    timer.set_after(self.delay);
                                    idle = false;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    now = FutureExt::fuse(&mut timer) => {
                        let mut wtime = DURATION_MAX;
                        while let Some((packet, time)) = b_tx_buffer.front() {
                            if *time <= now {
                                b_tx_buffer_size -= packet.len();
                                b.unbounded_send(b_tx_buffer.pop_front().unwrap().0);
                            } else {
                                let bwtime = time.duration_since(now);
                                if wtime > bwtime {
                                    wtime = bwtime;
                                }
                                break;
                            }
                        }
                        while let Some((packet, time)) = c_tx_buffer.front() {
                            if *time <= now {
                                c_tx_buffer_size -= packet.len();
                                c.unbounded_send(c_tx_buffer.pop_front().unwrap().0);
                            } else {
                                let cwtime = time.duration_since(now);
                                if wtime > cwtime {
                                    wtime = cwtime;
                                }
                                break;
                            }
                        }
                        timer.set_after(wtime);
                        idle = wtime == DURATION_MAX
                    }
                }
            }
        })
        .detach();
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_std::test]
    async fn test_delay() {
        let mut w = Wire::new();
        w.set_delay(Duration::from_millis(100));
        let (mut a, mut b) = w.spawn();
        let now = Instant::now();
        a.unbounded_send(vec![1]);
        a.unbounded_send(vec![2]);
        async_std::task::sleep(Duration::from_millis(10)).await;
        a.unbounded_send(vec![3]);
        a.unbounded_send(vec![4]);
        b.incoming().await;
        println!("{:?}", now.elapsed());
        assert!(now.elapsed() >= Duration::from_millis(100));
        assert!(now.elapsed() < Duration::from_millis(102));
        b.incoming().await;
        println!("{:?}", now.elapsed());
        assert!(now.elapsed() >= Duration::from_millis(100));
        assert!(now.elapsed() < Duration::from_millis(102));
        b.incoming().await;
        println!("{:?}", now.elapsed());
        assert!(now.elapsed() >= Duration::from_millis(110));
        assert!(now.elapsed() < Duration::from_millis(112));
        b.incoming().await;
        println!("{:?}", now.elapsed());
        assert!(now.elapsed() >= Duration::from_millis(110));
        assert!(now.elapsed() < Duration::from_millis(112));
    }
}
