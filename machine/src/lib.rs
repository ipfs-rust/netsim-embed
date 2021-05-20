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
use futures::future::FutureExt;
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

#[derive(Debug)]
enum IfaceCtrl {
    Up,
    Down,
    SetAddr(Ipv4Addr, u8, oneshot::Sender<()>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MachineId(pub usize);

/// Spawns a thread in a new network namespace and configures a TUN interface that sends and
/// receives IP packets from the tx/rx channels and runs some UDP/TCP networking code in task.
#[derive(Debug)]
pub struct Machine<C, E> {
    id: MachineId,
    addr: Ipv4Addr,
    mask: u8,
    ns: Namespace,
    ctrl: mpsc::UnboundedSender<IfaceCtrl>,
    tx: mpsc::UnboundedSender<C>,
    rx: mpsc::UnboundedReceiver<E>,
    join: Option<thread::JoinHandle<Result<()>>>,
}

impl<C, E> Machine<C, E>
where
    C: Display + Send + 'static,
    E: FromStr + Send + 'static,
    E::Err: std::fmt::Debug + Display + Send + Sync,
{
    pub async fn new(id: MachineId, plug: Plug, cmd: Command) -> Self {
        let (ctrl_tx, ctrl_rx) = mpsc::unbounded();
        let (cmd_tx, cmd_rx) = mpsc::unbounded();
        let (event_tx, event_rx) = mpsc::unbounded();
        let (ns_tx, ns_rx) = oneshot::channel();
        let join = machine(id, plug, cmd, ctrl_rx, ns_tx, cmd_rx, event_tx);
        let ns = ns_rx.await.unwrap();
        Self {
            id,
            addr: Ipv4Addr::UNSPECIFIED,
            mask: 32,
            ns,
            ctrl: ctrl_tx,
            tx: cmd_tx,
            rx: event_rx,
            join: Some(join),
        }
    }

    pub fn id(&self) -> MachineId {
        self.id
    }

    pub fn addr(&self) -> Ipv4Addr {
        self.addr
    }

    pub fn mask(&self) -> u8 {
        self.mask
    }

    pub async fn set_addr(&mut self, addr: Ipv4Addr, mask: u8) {
        let (tx, rx) = oneshot::channel();
        self.ctrl
            .unbounded_send(IfaceCtrl::SetAddr(addr, mask, tx))
            .unwrap();
        rx.await.unwrap();
        self.addr = addr;
        self.mask = mask;
    }

    pub fn send(&self, cmd: C) {
        self.tx.unbounded_send(cmd).unwrap();
    }

    pub async fn recv(&mut self) -> Option<E> {
        self.rx.next().await
    }

    pub fn up(&self) {
        self.ctrl.unbounded_send(IfaceCtrl::Up).unwrap();
    }

    pub fn down(&self) {
        self.ctrl.unbounded_send(IfaceCtrl::Down).unwrap();
    }

    pub fn namespace(&self) -> Namespace {
        self.ns
    }
}

impl<C, E> Drop for Machine<C, E> {
    fn drop(&mut self) {
        self.join.take().unwrap().join().unwrap().unwrap();
    }
}

#[allow(clippy::too_many_arguments)]
fn machine<C, E>(
    id: MachineId,
    plug: Plug,
    mut bin: Command,
    mut ctrl: mpsc::UnboundedReceiver<IfaceCtrl>,
    ns_tx: oneshot::Sender<Namespace>,
    mut cmd: mpsc::UnboundedReceiver<C>,
    event: mpsc::UnboundedSender<E>,
) -> thread::JoinHandle<Result<()>>
where
    C: Display + Send + 'static,
    E: FromStr + Send + 'static,
    E::Err: std::fmt::Debug + Display + Send + Sync,
{
    thread::spawn(move || {
        let ns = Namespace::unshare()?;
        let _ = ns_tx.send(ns);

        async_global_executor::block_on(async move {
            let iface = iface::Iface::new()?;
            let iface = async_io::Async::new(iface)?;
            let (mut tx, mut rx) = plug.split();

            let ctrl_task = async {
                while let Some(ctrl) = ctrl.next().await {
                    match ctrl {
                        IfaceCtrl::Up => iface.get_ref().put_up()?,
                        IfaceCtrl::Down => iface.get_ref().put_down()?,
                        IfaceCtrl::SetAddr(addr, mask, tx) => {
                            iface.get_ref().set_ipv4_addr(addr, mask)?;
                            iface.get_ref().put_up()?;
                            iface.get_ref().add_ipv4_route(Ipv4Range::global().into())?;
                            tx.send(()).ok();
                        }
                    }
                }
                Result::Ok(())
            }
            .fuse();
            futures::pin_mut!(ctrl_task);

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
                    log::debug!("machine {}: sending packet", id.0);
                    let mut bytes = buf[..n].to_vec();
                    if let Some(mut packet) = Packet::new(&mut bytes) {
                        packet.set_checksum();
                    }
                    if tx.send(bytes).await.is_err() {
                        break;
                    }
                }
                Result::Ok(())
            }
            .fuse();
            futures::pin_mut!(reader_task);

            let writer_task = async {
                while let Some(packet) = rx.next().await {
                    log::debug!("machine {}: received packet", id.0);
                    // can error if the interface is down
                    if let Ok(n) = iface.write_with(|iface| iface.send(&packet)).await {
                        if n == 0 {
                            break;
                        }
                    }
                }
                Result::Ok(())
            }
            .fuse();
            futures::pin_mut!(writer_task);

            bin.stdin(Stdio::piped()).stdout(Stdio::piped());
            let mut child = bin.spawn()?;
            let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines().fuse();
            let mut stdin = child.stdin.take().unwrap();

            let command_task = async {
                let mut buf = Vec::with_capacity(4096);
                while let Some(cmd) = cmd.next().await {
                    buf.clear();
                    tracing::trace!("{}", cmd);
                    writeln!(buf, "{}", cmd)?;
                    stdin.write_all(&buf).await?;
                }
                Result::Ok(())
            }
            .fuse();
            futures::pin_mut!(command_task);

            let event_task = async {
                while let Some(ev) = stdout.next().await {
                    let ev = ev?;
                    if ev.starts_with('<') {
                        let ev = match E::from_str(&ev) {
                            Ok(ev) => ev,
                            Err(err) => return Err(Error::new(ErrorKind::Other, err.to_string())),
                        };
                        if event.unbounded_send(ev).is_err() {
                            break;
                        }
                    } else {
                        println!("{}", ev);
                    }
                }
                Result::Ok(())
            }
            .fuse();
            futures::pin_mut!(event_task);

            futures::select! {
                res = ctrl_task => res?,
                res = reader_task => res?,
                res = writer_task => res?,
                res = command_task => res?,
                res = event_task => res?,
            }
            child.status().await.unwrap();
            Ok(())
        })
    })
}
