use smol_netsim_core::Ipv4Route;
use std::ffi::{CStr, CString};
use std::io::{self, Read, Write};
use std::mem;
use std::net::Ipv4Addr;
use std::os::unix::io::{AsRawFd, RawFd};

mod ioctl {
    use ioctl_sys::*;
    use libc::*;
    use std::ffi::CStr;
    use std::net::Ipv4Addr;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct ifreq {
        pub ifr_ifrn: __ifreq_ifr_ifrn,
        pub ifr_ifru: __ifreq_ifr_ifru,
    }

    impl ifreq {
        pub fn new(name: &CStr) -> Self {
            unsafe {
                let mut req: Self = std::mem::zeroed();
                std::ptr::copy_nonoverlapping(
                    name.as_ptr(),
                    req.ifr_ifrn.ifrn_name.as_mut_ptr() as *mut _,
                    name.to_bytes().len(),
                );
                req
            }
        }

        pub fn set_ifru_addr(&mut self, ipv4_addr: Ipv4Addr) {
            unsafe {
                let addr = &mut self.ifr_ifru.ifru_addr as *mut libc::sockaddr;
                let addr = &mut *(addr as *mut libc::sockaddr_in);
                addr.sin_family = libc::AF_INET as libc::sa_family_t;
                addr.sin_port = 0;
                addr.sin_addr.s_addr = u32::from(ipv4_addr).to_be();
            }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union __ifreq_ifr_ifrn {
        pub ifrn_name: [c_char; IFNAMSIZ],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union __ifreq_ifr_ifru {
        pub ifru_addr: sockaddr,
        pub ifru_dstaddr: sockaddr,
        pub ifru_broadaddr: sockaddr,
        pub ifru_netmask: sockaddr,
        pub ifru_hwaddr: sockaddr,
        pub ifru_flags: c_short,
        pub ifru_ivalue: c_int,
        pub ifru_mtu: c_int,
        pub ifru: ifmap,
        pub ifru_slave: [c_char; IFNAMSIZ],
        pub ifru_newname: [c_char; IFNAMSIZ],
        pub ifru_data: *mut c_void,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct ifmap {
        pub mem_start: c_ulong,
        pub mem_end: c_ulong,
        pub base_addr: c_ushort,
        pub irq: c_uchar,
        pub dma: c_uchar,
        pub port: c_uchar,
    }

    ioctl!(bad read siocgifflags with 0x8913; ifreq);
    ioctl!(bad write siocsifflags with 0x8914; ifreq);
    ioctl!(bad write siocsifaddr with 0x8916; ifreq);
    ioctl!(bad write siocsifnetmask with 0x891c; ifreq);
    ioctl!(write tunsetiff with b'T', 202; libc::c_int);
}

/// See: https://www.kernel.org/doc/Documentation/networking/tuntap.txt
pub struct Iface {
    name: CString,
    fd: RawFd,
}

impl AsRawFd for Iface {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl Drop for Iface {
    fn drop(&mut self) {
        let _ = unsafe { libc::close(self.as_raw_fd()) };
    }
}

impl Read for Iface {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv(buf)
    }
}

impl Write for Iface {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.send(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Iface {
    /// Creates a new virtual network interface.
    pub fn new() -> Result<Self, io::Error> {
        unsafe {
            let fd = loop {
                match errno!(libc::open(
                    b"/dev/net/tun\0".as_ptr() as *const _,
                    libc::O_RDWR
                )) {
                    Ok(fd) => break fd,
                    Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                    Err(err) => return Err(err),
                }
            };

            let mut req: ioctl::ifreq = mem::zeroed();
            req.ifr_ifru.ifru_flags = libc::IFF_TUN as i16 | libc::IFF_NO_PI as i16;

            errno!(ioctl::tunsetiff(fd, &mut req as *mut _ as *mut _))?;

            let name = CStr::from_ptr(&req.ifr_ifrn.ifrn_name as *const _).to_owned();

            Ok(Self { name, fd })
        }
    }

    /// Returns the name of the iface.
    pub fn name(&self) -> &CStr {
        &self.name
    }

    /// Receives a packet.
    pub fn recv(&self, buf: &mut [u8]) -> Result<usize, io::Error> {
        Ok(unsafe {
            errno!(libc::read(
                self.as_raw_fd(),
                buf.as_mut_ptr() as *mut _,
                buf.len()
            ))? as _
        })
    }

    /// Sends a packet.
    pub fn send(&self, buf: &[u8]) -> Result<usize, io::Error> {
        Ok(unsafe {
            errno!(libc::write(
                self.as_raw_fd(),
                buf.as_ptr() as *mut _,
                buf.len()
            ))? as _
        })
    }

    /// Set an interface IPv4 address and netmask
    pub fn set_ipv4_addr(&self, ipv4_addr: Ipv4Addr, netmask_bits: u8) -> Result<(), io::Error> {
        unsafe {
            let fd = errno!(libc::socket(
                libc::AF_INET as i32,
                libc::SOCK_DGRAM as i32,
                0
            ))?;
            let mut req = ioctl::ifreq::new(self.name());
            req.set_ifru_addr(ipv4_addr);

            if let Err(err) = errno!(ioctl::siocsifaddr(fd, &req)) {
                let _ = libc::close(fd);
                return Err(err);
            }

            let netmask = Ipv4Addr::from(!((!0u32) >> netmask_bits));
            req.set_ifru_addr(netmask);

            if let Err(err) = errno!(ioctl::siocsifnetmask(fd, &req)) {
                let _ = libc::close(fd);
                return Err(err);
            }

            let _ = libc::close(fd);
            Ok(())
        }
    }

    /// Put an interface up.
    pub fn put_up(&self) -> Result<(), io::Error> {
        unsafe {
            let fd = errno!(libc::socket(
                libc::AF_INET as i32,
                libc::SOCK_DGRAM as i32,
                0
            ))?;
            let mut req = ioctl::ifreq::new(self.name());

            if let Err(err) = errno!(ioctl::siocgifflags(fd, &mut req)) {
                let _ = libc::close(fd);
                return Err(err);
            }

            req.ifr_ifru.ifru_flags |= libc::IFF_UP as i16 | libc::IFF_RUNNING as i16;

            if let Err(err) = errno!(ioctl::siocsifflags(fd, &req)) {
                let _ = libc::close(fd);
                return Err(err);
            }

            let _ = libc::close(fd);
            Ok(())
        }
    }

    /// Adds an ipv4 route.
    pub fn add_ipv4_route(&self, route: Ipv4Route) -> Result<(), io::Error> {
        unsafe {
            let fd = errno!(libc::socket(
                libc::PF_INET as i32,
                libc::SOCK_DGRAM as i32,
                libc::IPPROTO_IP as i32,
            ))
            .unwrap();

            let mut rtentry: libc::rtentry = mem::zeroed();

            let rt_dst = &mut *(&mut rtentry.rt_dst as *mut _ as *mut libc::sockaddr_in);
            rt_dst.sin_family = libc::AF_INET as u16;
            rt_dst.sin_addr = libc::in_addr {
                s_addr: u32::from(route.dest().base_addr()).to_be(),
            };

            let rt_genmask = &mut *(&mut rtentry.rt_genmask as *mut _ as *mut libc::sockaddr_in);
            rt_genmask.sin_family = libc::AF_INET as u16;
            rt_genmask.sin_addr = libc::in_addr {
                s_addr: u32::from(route.dest().netmask()).to_be(),
            };

            rtentry.rt_flags = libc::RTF_UP as u16;

            if let Some(gateway_addr) = route.gateway() {
                let rt_gateway =
                    &mut *(&mut rtentry.rt_gateway as *mut _ as *mut libc::sockaddr_in);
                rt_gateway.sin_family = libc::AF_INET as u16;
                rt_gateway.sin_addr = libc::in_addr {
                    s_addr: u32::from(gateway_addr).to_be(),
                };

                rtentry.rt_flags |= libc::RTF_GATEWAY as u16;
            }

            rtentry.rt_dev = self.name.as_ptr() as *mut _;

            if let Err(err) = errno!(libc::ioctl(fd, u64::from(libc::SIOCADDRT), &rtentry)) {
                let _ = libc::close(fd);
                return Err(err).unwrap();
            }

            Ok(())
        }
    }
}
