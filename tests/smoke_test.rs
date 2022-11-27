use ipc_channel::ipc::{IpcReceiver, IpcSender};

#[no_mangle]
pub fn send_one(r: IpcReceiver<IpcSender<usize>>) {
    let sender = r.recv().unwrap();
    sender.send(1).unwrap();
}

#[no_mangle]
pub fn add(r: IpcReceiver<(IpcReceiver<usize>, IpcReceiver<usize>, IpcSender<usize>)>) {
    let (left, right, sender) = r.recv().unwrap();
    sender
        .send(left.recv().unwrap() + right.recv().unwrap())
        .unwrap();
}

fn can_send_one() {
    let mut s = netsim_embed::Netsim::<String, String>::new();
    let (sender, receiver) = ipc_channel::ipc::channel().unwrap();
    let (_, mach_sender) = async_std::task::block_on(s.spawn(send_one));
    mach_sender.send(sender).unwrap();
    assert_eq!(1, receiver.recv().unwrap());
}

fn one_plus_one_makes_two() {
    async_std::task::block_on(async {
        let mut s = netsim_embed::Netsim::<String, String>::new();

        let (sender1, receiver1) = ipc_channel::ipc::channel::<usize>().unwrap();
        let (_, mach_sender1) = s.spawn(send_one).await;
        mach_sender1.send(sender1).unwrap();

        let (sender2, receiver2) = ipc_channel::ipc::channel::<usize>().unwrap();
        let (_, mach_sender2) = s.spawn(send_one).await;
        mach_sender2.send(sender2).unwrap();

        let (sender3, receiver3) = ipc_channel::ipc::channel::<usize>().unwrap();
        let (_, mach_sender3) = s.spawn(add).await;
        mach_sender3.send((receiver1, receiver2, sender3)).unwrap();

        assert_eq!(2, receiver3.recv().unwrap());
    })
}

fn main() {
    netsim_embed::declare_machines!(send_one, add);
    netsim_embed::run_tests!(can_send_one, one_plus_one_makes_two);
}
