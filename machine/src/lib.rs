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
mod namespace;

pub use namespace::{unshare_user, Namespace};

use async_process::Command;
use futures::channel::{mpsc, oneshot};
use futures::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use netsim_embed_core::{Ipv4Range, Packet, Plug};
use std::fmt::Display;
use std::io::{Error, ErrorKind, Result, Write};
use std::net::Ipv4Addr;
use std::process::Stdio;
use std::str::FromStr;
use std::thread;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IfaceCtrl {
    Up,
    Down,
    SetAddr(Ipv4Addr, u8),
}

/// Spawns a thread in a new network namespace and configures a TUN interface that sends and
/// receives IP packets from the tx/rx channels and runs some UDP/TCP networking code in task.
pub fn machine<C, E>(
    addr: Ipv4Addr,
    mask: u8,
    plug: Plug,
    mut bin: Command,
    mut ctrl: mpsc::Receiver<IfaceCtrl>,
    ns_tx: oneshot::Sender<Namespace>,
    mut cmd: mpsc::UnboundedReceiver<C>,
    event: mpsc::UnboundedSender<E>,
) -> thread::JoinHandle<Result<()>>
where
    C: Display + Send + 'static,
    E: FromStr + Send + 'static,
    E::Err: std::fmt::Debug + std::error::Error + Send + Sync,
{
    thread::spawn(move || {
        tracing::trace!("spawning machine with addr {}", addr);
        let ns = Namespace::unshare()?;
        let _ = ns_tx.send(ns);

        async_global_executor::block_on(async move {
            let iface = iface::Iface::new()?;
            iface.set_ipv4_addr(addr, mask)?;
            iface.put_up().unwrap();
            iface.add_ipv4_route(Ipv4Range::global().into())?;

            let iface = async_io::Async::new(iface)?;
            let (mut tx, mut rx) = plug.split();

            let ctrl_task = async {
                while let Some(ctrl) = ctrl.next().await {
                    match ctrl {
                        IfaceCtrl::Up => iface.get_ref().put_up()?,
                        IfaceCtrl::Down => iface.get_ref().put_down()?,
                        IfaceCtrl::SetAddr(addr, mask) => {
                            iface.get_ref().set_ipv4_addr(addr, mask)?;
                        }
                    }
                }
                Result::Ok(())
            };

            let reader_task = async {
                loop {
                    let mut buf = [0; libc::ETH_FRAME_LEN as usize];
                    let n = iface.read_with(|iface| iface.recv(&mut buf)).await?;
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
                    if let Err(_) = tx.send(bytes).await {
                        break;
                    }
                }
                Result::Ok(())
            };

            let writer_task = async {
                while let Some(packet) = rx.next().await {
                    log::debug!("machine {}: received packet", addr);
                    // can error if the interface is down
                    if let Ok(n) = iface.write_with(|iface| iface.send(&packet)).await {
                        if n == 0 {
                            break;
                        }
                    }
                }
                Result::Ok(())
            };

            bin.stdin(Stdio::piped()).stdout(Stdio::piped());
            let mut child = bin.spawn()?;
            let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines().fuse();
            let mut stdin = child.stdin.unwrap();

            let command_task = async {
                let mut buf = Vec::with_capacity(4096);
                while let Some(cmd) = cmd.next().await {
                    buf.clear();
                    tracing::trace!("{}", cmd);
                    writeln!(buf, "{}", cmd)?;
                    stdin.write_all(&buf).await?;
                }
                Result::Ok(())
            };

            let event_task = async {
                while let Some(ev) = stdout.next().await {
                    let ev = ev?;
                    if ev.starts_with('<') {
                        let ev = match ev.parse() {
                            Ok(ev) => ev,
                            Err(err) => return Err(Error::new(ErrorKind::Other, err)),
                        };
                        if let Err(_) = event.unbounded_send(ev) {
                            break;
                        }
                    } else {
                        println!("{}", ev);
                    }
                }
                Result::Ok(())
            };

            let (res1, res2, res3, res4, res5) = futures::future::join5(
                ctrl_task,
                reader_task,
                writer_task,
                command_task,
                event_task,
            )
            .await;
            res1?;
            res2?;
            res3?;
            res4?;
            res5?;
            Ok(())
        })
    })
}
