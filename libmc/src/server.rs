use crate::*;

#[derive(Debug, Copy, Clone)]
pub struct ClientID(u32);

pub trait Server {
    fn on_connect(&mut self, cid: ClientID);
    fn on_disconnect(&mut self, cid: ClientID);
    fn handle_packet(&mut self, cid: ClientID, packet: InPacket);
}

pub fn run_server<S: Server>(mut s: S) {
    let mut todo_cid = ClientID(0);

    let (stream, _) = std::net::TcpListener::bind("127.0.0.1:25565")
        .unwrap()
        .accept()
        .unwrap();
    let stream = &stream;
    let mut ps = PacketReader::new(std::io::BufReader::new(stream));

    // TODO: multiple clients (increment cid)
    s.on_connect(todo_cid);

    loop {
        s.handle_packet(todo_cid, pr.next_packet());
        OutPacket::Disconnect {
            reason: "you're a doodoo head",
        }
        .serialize_to(stream);
    }

    // TODO: multiple clients (increment cid)
    s.on_disconnect(todo_cid);
}
