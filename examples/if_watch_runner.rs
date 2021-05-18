use netsim_embed::{run, Ipv4Range, Network, Wire};

fn main() {
    env_logger::init();
    run(async {
        let mut net = Network::<String, String>::new(Ipv4Range::global());
        let addr1 = net.random_client_addr();
        let addr2 = net.random_client_addr();
        let addr3 = net.random_client_addr();
        let if_watch_bin = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("if_watch");
        net.spawn_machine(
            Wire::new(),
            Some(addr1),
            async_process::Command::new(if_watch_bin),
        )
        .await;
        let machine = net.machine(0);
        assert_eq!(machine.recv().await, Some(format!("<up {}/0", addr1)));
        machine.set_addr(addr2);
        assert_eq!(machine.recv().await, Some(format!("<down {}/0", addr1)));
        assert_eq!(machine.recv().await, Some(format!("<up {}/32", addr2)));
        assert_eq!(machine.recv().await, Some(format!("<down {}/32", addr2)));
        assert_eq!(machine.recv().await, Some(format!("<up {}/0", addr2)));
        machine.set_addr(addr3);
        assert_eq!(machine.recv().await, Some(format!("<down {}/0", addr2)));
        assert_eq!(machine.recv().await, Some(format!("<up {}/32", addr3)));
        assert_eq!(machine.recv().await, Some(format!("<down {}/32", addr3)));
        assert_eq!(machine.recv().await, Some(format!("<up {}/0", addr3)));
    });
}
