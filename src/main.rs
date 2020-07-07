use crate::iface::Iface;
use futures::channel::mpsc::{self, Receiver, Sender};
use futures::future::Future;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std::fs::File;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::{io, thread};

mod iface;

pub fn unshare_user() -> Result<(), io::Error> {
    let uid = unsafe { libc::geteuid() };
    let gid = unsafe { libc::getegid() };

    unsafe { errno!(libc::unshare(libc::CLONE_NEWUSER))? };

    let mut f = File::create("/proc/self/uid_map")?;
    let s = format!("0 {} 1\n", uid);
    f.write(s.as_bytes())?;

    let mut f = File::create("/proc/self/setgroups")?;
    f.write(b"deny\n")?;

    let mut f = File::create("/proc/self/gid_map")?;
    let s = format!("0 {} 1\n", gid);
    f.write(s.as_bytes())?;

    Ok(())
}

/// Spawns a thread in a new network namespace and configures a TUN interface that sends and
/// receives IP packets from the tx/rx channels and runs some UDP/TCP networking code in task.
pub fn machine<F>(
    addr: Ipv4Addr,
    mask: u8,
    mut tx: Sender<Vec<u8>>,
    mut rx: Receiver<Vec<u8>>,
    task: F,
) -> thread::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    thread::spawn(move || {
        unsafe {
            errno!(libc::unshare(libc::CLONE_NEWNET | libc::CLONE_NEWUTS)).unwrap();
        }
        let iface = Iface::new().unwrap();
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

fn main() {
    unshare_user().unwrap();
    let a_addr: Ipv4Addr = "192.168.1.5".parse().unwrap();
    let b_addr = "192.168.1.6".parse().unwrap();
    let (a_tx, b_rx) = mpsc::channel(0);
    let (b_tx, a_rx) = mpsc::channel(0);

    let join1 = machine(a_addr.clone(), 24, a_tx, a_rx, async move {
        let socket = smol::Async::<UdpSocket>::bind("0.0.0.0:3000").unwrap();
        loop {
            let mut buf = [0u8; 11];
            let (len, addr) = socket.recv_from(&mut buf).await.unwrap();
            if &buf[..len] == b"ping" {
                println!("received ping");
                socket.send_to(b"pong", addr).await.unwrap();
                break;
            }
        }
    });

    let join2 = machine(b_addr, 24, b_tx, b_rx, async move {
        let socket = smol::Async::<UdpSocket>::bind("0.0.0.0:3000").unwrap();
        socket
            .send_to(b"ping", SocketAddrV4::new(a_addr, 3000))
            .await
            .unwrap();

        let mut buf = [0u8; 11];
        let (len, _addr) = socket.recv_from(&mut buf).await.unwrap();
        if &buf[..len] == b"pong" {
            println!("received pong");
        }
    });

    join1.join().unwrap();
    join2.join().unwrap();
}
