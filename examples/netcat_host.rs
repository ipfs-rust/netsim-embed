use async_process::Command;
use netsim_embed::*;
use netsim_embed_machine::Namespace;

fn main() {
    run(async {
        env_logger::init();
        let mut netsim = Netsim::<String, String>::new();
        let net1 = netsim.spawn_network(Ipv4Range::global());
        let mut server = Command::new("ncat");
        server.args(["-l", "-4", "-p", "4242", "-c", "echo '<Hello World'"]);

        let server = netsim.spawn_machine(server, None).await;
        netsim.plug(server, net1, None).await;
        let server_addr = netsim.machine(server).addr();
        println!("Server Addr {server_addr}:4242");

        let _ns = Namespace::current().unwrap();
        let ns_server = netsim.machine(server).namespace();
        println!("{ns_server}");
        netsim.machine(server).namespace().enter().unwrap();

        let mut cmd = Command::new("nc");
        cmd.args([&*server_addr.to_string(), "4242"]);
        let output = cmd.output().await.unwrap();
        println!("response: {}", std::str::from_utf8(&output.stdout).unwrap());
        //println!("error: {}", std::str::from_utf8(&output.stderr).unwrap());
    });
}
