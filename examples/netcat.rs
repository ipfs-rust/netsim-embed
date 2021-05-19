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
        env_logger::init();
        let mut netsim = Netsim::<HelloWorld, HelloWorld>::new();
        let net1 = netsim.spawn_network(Ipv4Range::global().split(2)[0]);
        let net2 = netsim.spawn_network(Ipv4Range::global().split(2)[1]);
        let mut server = Command::new("nc");
        server.args(&["-l", "-4", "-p", "4242"]);

        let server = netsim.spawn_machine(server, None).await;
        netsim.plug(server, net1, None).await;
        let server_addr = netsim.machine(server).addr();
        println!("Server Addr {}:4242", server_addr.to_string());

        let mut client = Command::new("nc");
        client.args(&["-4", &*server_addr.to_string(), "4242"]);
        let client = netsim.spawn_machine(client, None).await;
        netsim.plug(client, net2, None).await;
        netsim.add_route(net1, net2);

        netsim.machine(server).send(HelloWorld);
        let str = netsim.machine(client).recv().await.unwrap();
        println!("{}", str);
    });
}
