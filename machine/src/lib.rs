//! Small embeddable network simulator.

macro_rules! errno {
    ($res:expr) => {{
        let res = $res;
        if res < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}

pub mod iface;
pub mod namespace;

use futures::future::Future;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use netsim_embed_core::{Ipv4Range, Packet, Plug};
use std::net::Ipv4Addr;
use std::thread;

/// Spawns a thread in a new network namespace and configures a TUN interface that sends and
/// receives IP packets from the tx/rx channels and runs some UDP/TCP networking code in task.
pub fn machine<F>(addr: Ipv4Addr, mask: u8, plug: Plug, task: F) -> thread::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    thread::spawn(move || {
        tracing::trace!("spawning machine with addr {}", addr);
        namespace::unshare_network().unwrap();

        async_global_executor::block_on(async move {
            let iface = iface::Iface::new().unwrap();
            iface.set_ipv4_addr(addr, mask).unwrap();
            iface.put_up().unwrap();
            iface.add_ipv4_route(Ipv4Range::global().into()).unwrap();

            let iface = async_io::Async::new(iface).unwrap();

            let (mut tx, mut rx) = plug.split();
            let (mut reader, mut writer) = iface.split();

            let reader_task = async move {
                loop {
                    let mut buf = [0; libc::ETH_FRAME_LEN as usize];
                    let n = reader.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    // drop ipv6 packets
                    if buf[0] >> 4 != 4 {
                        continue;
                    }
                    log::debug!("machine {}: sending packet", addr);
                    let mut bytes = buf[..n].to_vec();
                    if let Some(mut packet) = Packet::new(&mut bytes) {
                        packet.set_checksum();
                    }
                    if tx.send(bytes).await.is_err() {
                        break;
                    }
                }
            };

            let writer_task = async move {
                while let Some(packet) = rx.next().await {
                    log::debug!("machine {}: received packet", addr);
                    let n = writer.write(&packet).await.unwrap();
                    if n == 0 {
                        break;
                    }
                }
            };

            async_global_executor::spawn(reader_task).detach();
            async_global_executor::spawn(writer_task).detach();
            task.await
        })
    })
}
