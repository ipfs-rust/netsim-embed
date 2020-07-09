# netsim-embed
A small embeddable network simulator.

Run the example:

```sh
cargo run --example smol
    Finished dev [unoptimized + debuginfo] target(s) in 0.03s
     Running `target/debug/examples/smol`
[2020-07-09T16:09:17Z TRACE netsim_embed_machine::namespace] created network namespace: /proc/10753/task/10850/ns/net
[2020-07-09T16:09:17Z TRACE netsim_embed_machine::namespace] created network namespace: /proc/10753/task/10849/ns/net
[2020-07-09T16:09:17Z INFO  netsim_embed_machine] machine 192.168.1.5: sending packet
[2020-07-09T16:09:17Z INFO  netsim_embed_machine] machine 8.8.8.4: sending packet
[2020-07-09T16:09:17Z INFO  netsim_embed_machine] machine 192.168.1.5: sending packet
[2020-07-09T16:09:17Z INFO  netsim_embed_nat::nat] nat 8.8.8.8: dropping invalid outbound packet
[2020-07-09T16:09:17Z INFO  netsim_embed_router] router 8.8.8.1: dropping unroutable packet to 71.23.170.98
[2020-07-09T16:09:17Z INFO  netsim_embed_router] router 8.8.8.1: dropping unroutable packet to 71.23.170.98
[2020-07-09T16:09:17Z INFO  netsim_embed_nat::nat] nat 8.8.8.8: rewrote packet source address: 192.168.1.5:33542 => 8.8.8.8:49152
[2020-07-09T16:09:17Z INFO  netsim_embed_router] router 8.8.8.1: dropping unroutable packet to 8.8.8.4
[2020-07-09T16:09:17Z INFO  netsim_embed_router] router 8.8.8.1: routing packet on route Ipv4Route { dest: 8.8.8.4/32, gateway: None }
[2020-07-09T16:09:17Z INFO  netsim_embed_machine] machine 8.8.8.4: received packet
received ping
[2020-07-09T16:09:17Z INFO  netsim_embed_machine] machine 8.8.8.4: sending packet
[2020-07-09T16:09:17Z INFO  netsim_embed_router] router 8.8.8.1: routing packet on route Ipv4Route { dest: 8.8.8.8/32, gateway: None }
[2020-07-09T16:09:17Z INFO  netsim_embed_nat::nat] nat 8.8.8.8: rewrote destination of inbound packet 8.8.8.8:49152 => 192.168.1.5:33542.
[2020-07-09T16:09:17Z INFO  netsim_embed_machine] machine 192.168.1.5: received packet
received pong
```

Enter the network namespace and debug dropped packets with netstat:

```sh
sudo nsenter --net=/proc/10753/task/10850/ns/net
[root@dvc-xps13-2020 dvc]# netstat -suna
Udp:
    0 packets received
    0 packets to unknown port received
    0 packet receive errors
    1 packets sent
    0 receive buffer errors
    0 send buffer errors
UdpLite:
IpExt:
    OutOctets: 32
MPTcpExt:
```

## License
MIT OR Apache-2.0
