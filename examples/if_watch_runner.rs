use netsim_embed::{run, Ipv4Range, Netsim};

fn main() {
    env_logger::init();
    run(async {
        let mut sim = Netsim::<String, String>::new();
        let net = sim.spawn_network(Ipv4Range::global());
        let addr1 = sim.network(net).random_addr();
        let addr2 = sim.network(net).random_addr();
        let addr3 = sim.network(net).random_addr();
        let if_watch_bin = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("if_watch");
        let machine = sim
            .spawn_machine(async_process::Command::new(if_watch_bin), None)
            .await;
        sim.plug(machine, net, Some(addr1)).await;
        let machine = &mut sim.machines_mut()[0];
        assert_eq!(machine.recv().await, Some(format!("<up {}/0", addr1)));
        machine.set_addr(addr2, 0);
        assert_eq!(machine.recv().await, Some(format!("<down {}/0", addr1)));
        assert_eq!(machine.recv().await, Some(format!("<up {}/32", addr2)));
        assert_eq!(machine.recv().await, Some(format!("<down {}/32", addr2)));
        assert_eq!(machine.recv().await, Some(format!("<up {}/0", addr2)));
        machine.set_addr(addr3, 0);
        assert_eq!(machine.recv().await, Some(format!("<down {}/0", addr2)));
        assert_eq!(machine.recv().await, Some(format!("<up {}/32", addr3)));
        assert_eq!(machine.recv().await, Some(format!("<down {}/32", addr3)));
        assert_eq!(machine.recv().await, Some(format!("<up {}/0", addr3)));
    });
}
