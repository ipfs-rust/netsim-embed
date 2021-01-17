use std::fs::File;
use std::io::{self, Write};

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

pub fn unshare_network() -> Result<(), io::Error> {
    unsafe {
        errno!(libc::unshare(libc::CLONE_NEWNET | libc::CLONE_NEWUTS))?;
        let pid = errno!(libc::getpid())?;
        let tid = errno!(libc::syscall(libc::SYS_gettid))?;
        log::info!(
            "created network namespace: /proc/{}/task/{}/ns/net",
            pid,
            tid
        );
        Ok(())
    }
}
