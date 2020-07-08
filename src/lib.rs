//! Smol network simulator.

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

use futures::channel::mpsc;
use futures::future::Future;
use futures::stream::StreamExt;
use futures::sink::SinkExt;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use std::net::Ipv4Addr;
use std::thread;

/// Spawns a thread in a new network namespace and configures a TUN interface that sends and
/// receives IP packets from the tx/rx channels and runs some UDP/TCP networking code in task.
pub fn machine<F>(
    addr: Ipv4Addr,
    mask: u8,
    mut tx: mpsc::Sender<Vec<u8>>,
    mut rx: mpsc::Receiver<Vec<u8>>,
    task: F,
) -> thread::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    thread::spawn(move || {
        namespace::unshare_network().unwrap();
        let iface = iface::Iface::new().unwrap();
        iface.set_ipv4_addr(addr, mask).unwrap();
        iface.put_up().unwrap();
        let iface = smol::Async::new(iface).unwrap();
        let (mut reader, mut writer) = iface.split();

        smol::run(async move {
            smol::Task::local(async move {
                loop {
                    let mut buf = [0; libc::ETH_FRAME_LEN as usize];
                    let n = reader.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    if tx.send(buf[..n].to_vec()).await.is_err() {
                        break;
                    }
                }
            })
            .detach();

            smol::Task::local(async move {
                loop {
                    if let Some(packet) = rx.next().await {
                        let n = writer.write(&packet).await.unwrap();
                        if n == 0 {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            })
            .detach();

            task.await
        })
    })
}
