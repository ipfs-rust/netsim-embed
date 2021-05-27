use std::fs::File;
use std::io::{self, Write};
use std::marker::PhantomData;
use std::os::unix::io::AsRawFd;

pub fn unshare_user() -> Result<(), io::Error> {
    let uid = unsafe { libc::geteuid() };
    let gid = unsafe { libc::getegid() };

    unsafe { errno!(libc::unshare(libc::CLONE_NEWUSER))? };

    let mut f = File::create("/proc/self/uid_map")?;
    let s = format!("0 {} 1\n", uid);
    f.write_all(s.as_bytes())?;

    let mut f = File::create("/proc/self/setgroups")?;
    f.write_all(b"deny\n")?;

    let mut f = File::create("/proc/self/gid_map")?;
    let s = format!("0 {} 1\n", gid);
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

    /// Enter this namespace and change back to previous one when guard is dropped
    pub fn enter_guarded(&self) -> Result<NamespaceGuard, io::Error> {
        let prior_ns = Self::current()?;
        self.enter()?;
        Ok(NamespaceGuard {
            prior_ns,
            _ph: PhantomData,
        })
    }
}

impl std::fmt::Display for Namespace {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "/proc/{}/task/{}/ns/net", self.pid, self.tid)
    }
}

pub struct NamespaceGuard {
    prior_ns: Namespace,
    _ph: PhantomData<std::rc::Rc<()>>,
}

impl Drop for NamespaceGuard {
    fn drop(&mut self) {
        if let Err(e) = self.prior_ns.enter() {
            log::error!("cannot change back to namespace {}: {}", self.prior_ns, e);
        }
    }
}
