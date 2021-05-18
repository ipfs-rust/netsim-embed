use async_process::{Command, Stdio};
use async_std::io::BufReader;
use futures::{AsyncBufReadExt, StreamExt};
use netsim_embed::*;
use tracing::info;

fn main() {
    run(async {
        env_logger::init();
        let mut net = NetworkBuilder::<String, String>::new(Ipv4Range::global());
        let server_addr = net.random_client_addr();
        println!("Server Addr {}:4242", server_addr.to_string());
        let mut server = Command::new("nc");
        server.args(&["-l", "-4", "-p", "4242", "-c", "echo -e 'Hello World'"]);

        net.spawn_machine(Wire::new(), Some(server_addr), server);
        let mut host = net.spawn_host(None);
        let _network = net.spawn();

        let fut = async move {
            let mut cmd = Command::new("nc");

            cmd.args(&["-4", &*server_addr.to_string(), "4242"]);
            cmd.stdout(Stdio::piped());
            let mut child = cmd.spawn().unwrap();
            info!("Spawned child");
            let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines().fuse();
            let line = stdout.next().await.unwrap()?;
            child.kill()?;

            Result::<_, anyhow::Error>::Ok(line)
        };
        let response = host.run(fut).await.unwrap().unwrap();
        println!("response: {}", response);
    });
}
