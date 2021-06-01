fn main() -> anyhow::Result<()> {
    println!("checking user namespace support");
    netsim_embed::unshare_user()?;
    println!("user namespace works");
    println!("checking network namespace support");
    netsim_embed::Namespace::unshare()?;
    println!("network namespace works");
    println!("checking tun adapter support");
    netsim_embed_machine::iface::Iface::new()?;
    println!("tun adapter works");
    println!("all set, your system seems to be working");
    Ok(())
}
