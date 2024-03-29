use std::fs::File;
use std::io::{self, Write};
use std::os::unix::io::AsRawFd;

pub fn unshare_user() -> Result<(), io::Error> {
    let uid = unsafe { libc::geteuid() };
    let gid = unsafe { libc::getegid() };

    unsafe { errno!(libc::unshare(libc::CLONE_NEWUSER))? };

    let mut f = File::create("/proc/self/uid_map")?;
    let s = format!("0 {uid} 1\n");
    f.write_all(s.as_bytes())?;

    let mut f = File::create("/proc/self/setgroups")?;
    f.write_all(b"deny\n")?;

    let mut f = File::create("/proc/self/gid_map")?;
    let s = format!("0 {gid} 1\n");
    f.write_all(s.as_bytes())?;

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Namespace {
    pid: i32,
    tid: i64,
}

impl Namespace {
    pub fn current() -> Result<Self, io::Error> {
        unsafe {
            let pid = errno!(libc::getpid())?;
            let tid = errno!(libc::syscall(libc::SYS_gettid))?;
            Ok(Self { pid, tid })
        }
    }

    pub fn unshare() -> Result<Self, io::Error> {
        unsafe {
            errno!(libc::unshare(libc::CLONE_NEWNET | libc::CLONE_NEWUTS))?;
        }
        Self::current()
    }

    pub fn enter(&self) -> Result<(), io::Error> {
        let fd = File::open(self.to_string())?;
        unsafe {
            errno!(libc::setns(fd.as_raw_fd(), libc::CLONE_NEWNET))?;
        }
        Ok(())
    }
}

impl std::fmt::Display for Namespace {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "/proc/{}/task/{}/ns/net", self.pid, self.tid)
    }
}
