use async_process::Command;
use netsim_embed::*;
use std::{
    fmt::{self, Display},
    str::FromStr,
};

struct HelloWorld;
impl FromStr for HelloWorld {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "<Hello World" {
            Ok(Self)
        } else {
            Err("??".into())
        }
    }
}
impl Display for HelloWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<Hello World")
    }
}
fn main() {
    run(async {
        let mut net = NetworkBuilder::<HelloWorld, HelloWorld>::new(Ipv4Range::global());
        let server_addr = net.random_client_addr();
        let mut server = Command::new("nc");
        server.args(&["-l", "-4", "-p", "4242"]);

        net.spawn_machine(Wire::new(), Some(server_addr), server);

        let mut local =
            NetworkBuilder::<HelloWorld, HelloWorld>::new(Ipv4Range::random_local_subnet());
        let mut client = Command::new("nc");
        client.args(&["-4", &*server_addr.to_string(), "4242"]);
        local.spawn_machine(Wire::new(), None, client);

        net.spawn_network(Some(NatConfig::default()), local);

        let mut network = net.spawn();
        network.machine(0).send(HelloWorld).await;
        let str = network.subnet(0).machine(0).recv().await.unwrap();
        println!("{}", str);
    });
}
