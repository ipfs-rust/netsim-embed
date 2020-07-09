use netsim_embed::{run_star, NatConfig, StarConfig};
use ipfs_embed::{Config, Store};

fn main() {
    let mut config = StarConfig::default();
    config.num_public = 2;

    run_star(config, |_net, node| {
        let tmp = TempDir::new("netsim_embed");
        let config = Config::from_path(tmp.as_path());
        let store = Store::new(config);
        store.insert(node).await.unwrap();
        store.get((node + 1) % 2).await.unwrap();
    })
}
