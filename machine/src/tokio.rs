use crate::iface::Iface;
use futures::io::{AsyncRead, AsyncWrite};
use std::io;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead as _, AsyncWrite as _, PollEvented};

pub struct TokioFd(PollEvented<Iface>);

impl TokioFd {
    pub fn new(iface: Iface) -> Result<Self, io::Error> {
        let fd = iface.as_raw_fd();

        let flags = unsafe { errno!(libc::fcntl(fd, libc::F_GETFL, 0))? };

        unsafe { errno!(libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK))? };

        Ok(Self(PollEvented::new(iface)?))
    }
}

impl mio::Evented for Iface {
    fn register(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> io::Result<()> {
        let fd = self.as_raw_fd();
        let evented_fd = mio::unix::EventedFd(&fd);
        evented_fd.register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> io::Result<()> {
        let fd = self.as_raw_fd();
        let evented_fd = mio::unix::EventedFd(&fd);
        evented_fd.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        let fd = self.as_raw_fd();
        let evented_fd = mio::unix::EventedFd(&fd);
        evented_fd.deregister(poll)
    }
}

impl AsyncRead for TokioFd {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for TokioFd {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}
