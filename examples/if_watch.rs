use anyhow::Result;
use if_watch::{IfEvent, IfWatcher};
use ipnet::IpNet;

#[async_std::main]
async fn main() -> Result<()> {
    let mut watcher = IfWatcher::new().await?;
    loop {
        let watcher = &mut watcher;
        match watcher.await? {
            IfEvent::Down(IpNet::V4(ip)) => {
                println!("down {ip}");
                println!("<down {ip}");
            }
            IfEvent::Up(IpNet::V4(ip)) => {
                println!("up {ip}");
                println!("<up {ip}");
            }
            x => {
                println!("other event {x:?}");
            }
        }
    }
}
