use async_process::{Command, Stdio};
use async_std::io::BufReader;
use futures::{AsyncBufReadExt, StreamExt};
use netsim_embed::*;
use netsim_embed_machine::Namespace;

fn main() {
    run(async {
        env_logger::init();
        let mut netsim = Netsim::<String, String>::new();
        let net1 = netsim.spawn_network(Ipv4Range::global());
        let mut server = Command::new("nc");
        server.args(&["-l", "-4", "-p", "4242", "-c", "echo -e '<Hello World'"]);

        let server = netsim.spawn_machine(server, None).await;
        netsim.plug(server, net1, None).await;
        let server_addr = netsim.machine(server).addr();
        println!("Server Addr {}:4242", server_addr.to_string());

        let fut = async move {
            let mut cmd = Command::new("nc");

            cmd.args(&["-4", &*server_addr.to_string(), "4242"]);
            cmd.stdout(Stdio::piped());
            let mut child = cmd.spawn().unwrap();
            println!("Spawned child");
            let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines().fuse();
            let line = stdout.next().await.unwrap()?;
            child.kill()?;

            Result::<_, anyhow::Error>::Ok(line)
        };
        let _ns = Namespace::current().unwrap();
        let ns_server = netsim.machine(server).namespace();
        println!("{}", ns_server);
        netsim.machine(server).namespace().enter().unwrap();
        let response = fut.await.unwrap();
        println!("response: {}", response);
    });
}
