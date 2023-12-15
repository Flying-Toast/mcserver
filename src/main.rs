use libmc::*;

struct BasicServer {}

impl Server for BasicServer {
    fn on_connect(&mut self, cid: ClientID) {}

    fn on_disconnect(&mut self, cid: ClientID) {}

    fn handle_packet(&mut self, cid: ClientID, packet: InPacket) {
        dbg!(packet);
    }
}

fn main() {
    // TODO: this loop temporary until libmc handles multiple clients with async
    loop {
        run_server(BasicServer {})
    }
}
