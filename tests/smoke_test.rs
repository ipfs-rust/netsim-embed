use ipc_channel::ipc::{IpcReceiver, IpcSender};

#[netsim_embed::machine]
fn send_one(sender: IpcSender<usize>) {
    sender.send(1).unwrap();
}

#[netsim_embed::machine]
fn add((left, right, sender): (IpcReceiver<usize>, IpcReceiver<usize>, IpcSender<usize>)) {
    sender
        .send(left.recv().unwrap() + right.recv().unwrap())
        .unwrap();
}

fn can_send_one() {
    let mut s = netsim_embed::Netsim::<String, String>::new();
    let (sender, receiver) = ipc_channel::ipc::channel().unwrap();
    let _ = async_std::task::block_on(s.spawn(send_one, sender));
    assert_eq!(1, receiver.recv().unwrap());
}

fn one_plus_one_makes_two() {
    async_std::task::block_on(async {
        let mut s = netsim_embed::Netsim::<String, String>::new();

        let (sender1, receiver1) = ipc_channel::ipc::channel::<usize>().unwrap();
        let _ = s.spawn(send_one, sender1).await;

        let (sender2, receiver2) = ipc_channel::ipc::channel::<usize>().unwrap();
        let _ = s.spawn(send_one, sender2).await;

        let (sender3, receiver3) = ipc_channel::ipc::channel::<usize>().unwrap();
        let _ = s.spawn(add, (receiver1, receiver2, sender3)).await;

        assert_eq!(2, receiver3.recv().unwrap());
    })
}

fn main() {
    netsim_embed::declare_machines!(send_one, add);
    netsim_embed::run_tests!(can_send_one, one_plus_one_makes_two);
}
