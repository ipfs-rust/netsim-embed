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

use async_process::Command;
use futures::channel::mpsc;
use futures::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use netsim_embed_core::{Ipv4Range, Packet, Plug};
use std::fmt::Display;
use std::io::Write;
use std::net::Ipv4Addr;
use std::process::Stdio;
use std::str::FromStr;
use std::thread;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IfaceCtrl {
    Up,
    Down,
}

/// Spawns a thread in a new network namespace and configures a TUN interface that sends and
/// receives IP packets from the tx/rx channels and runs some UDP/TCP networking code in task.
pub fn machine<C, E>(
    addr: Ipv4Addr,
    mask: u8,
    plug: Plug,
    mut bin: Command,
    mut ctrl: mpsc::Receiver<IfaceCtrl>,
    mut cmd: mpsc::UnboundedReceiver<C>,
    event: mpsc::UnboundedSender<E>,
) -> thread::JoinHandle<()>
where
    C: Display + Send + 'static,
    E: FromStr + Send + 'static,
    E::Err: std::fmt::Debug + Send,
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

            let ctrl_task = async {
                while let Some(ctrl) = ctrl.next().await {
                    match ctrl {
                        IfaceCtrl::Up => iface.get_ref().put_up().unwrap(),
                        IfaceCtrl::Down => iface.get_ref().put_down().unwrap(),
                    }
                }
            };

            let reader_task = async {
                loop {
                    let mut buf = [0; libc::ETH_FRAME_LEN as usize];
                    let n = iface.read_with(|iface| iface.recv(&mut buf)).await.unwrap();
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

            let writer_task = async {
                while let Some(packet) = rx.next().await {
                    log::debug!("machine {}: received packet", addr);
                    let n = iface.write_with(|iface| iface.send(&packet)).await.unwrap();
                    if n == 0 {
                        break;
                    }
                }
            };

            bin.stdin(Stdio::piped()).stdout(Stdio::piped());
            let mut child = bin.spawn().unwrap();
            let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines().fuse();
            let mut stdin = child.stdin.unwrap();

            let command_task = async {
                let mut buf = Vec::with_capacity(4096);
                while let Some(cmd) = cmd.next().await {
                    buf.clear();
                    tracing::trace!("{}", cmd);
                    writeln!(buf, "{}", cmd).unwrap();
                    stdin.write_all(&buf).await.unwrap();
                }
            };

            let event_task = async {
                while let Some(ev) = stdout.next().await {
                    let ev = ev.unwrap();
                    if ev.starts_with('<') {
                        event.unbounded_send(ev.parse().unwrap()).unwrap();
                    } else {
                        println!("{}", ev);
                    }
                }
            };

            futures::future::join5(
                ctrl_task,
                reader_task,
                writer_task,
                command_task,
                event_task,
            )
            .await;
        })
    })
}
