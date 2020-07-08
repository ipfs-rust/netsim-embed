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
#[cfg(feature = "tokio2")]
pub mod tokio;

use futures::future::Future;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use smol_netsim_core::Plug;
use std::net::Ipv4Addr;
use std::thread;

/// Spawns a thread in a new network namespace and configures a TUN interface that sends and
/// receives IP packets from the tx/rx channels and runs some UDP/TCP networking code in task.
pub fn machine<F>(
    addr: Ipv4Addr,
    mask: u8,
    plug: Plug,
    task: F,
) -> thread::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    thread::spawn(move || {
        namespace::unshare_network().unwrap();

        let create_tun_iface = || {
            let iface = iface::Iface::new().unwrap();
            iface.set_ipv4_addr(addr, mask).unwrap();
            iface.put_up().unwrap();

            #[cfg(not(feature = "tokio2"))]
            let iface = smol::Async::new(iface).unwrap();
            #[cfg(feature = "tokio2")]
            let iface = tokio::TokioFd::new(iface).unwrap();

            let (mut tx, mut rx) = plug.split();
            let (mut reader, mut writer) = iface.split();

            let reader_task = async move {
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
            };

            let writer_task = async move {
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
            };

            (reader_task, writer_task)
        };

        #[cfg(not(feature = "tokio2"))]
        let result = smol::run(async move {
            let (reader_task, writer_task) = create_tun_iface();
            smol::Task::spawn(reader_task).detach();
            smol::Task::spawn(writer_task).detach();
            task.await
        });
        #[cfg(feature = "tokio2")]
        let result = {
            let mut rt = ::tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let (reader_task, writer_task) = create_tun_iface();
                ::tokio::task::spawn(reader_task);
                ::tokio::task::spawn(writer_task);
                task.await
            })
        };

        result
    })
}
