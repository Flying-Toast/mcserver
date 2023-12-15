use crate::*;

#[derive(Debug, Copy, Clone)]
pub struct ClientID(u32);

pub trait Server {
    fn on_connect(&mut self, cid: ClientID);
    fn on_disconnect(&mut self, cid: ClientID);
    fn handle_packet(&mut self, cid: ClientID, packet: InPacket);
}

pub fn run_server<S: Server>(mut s: S) {
    let todo_cid = ClientID(0);

    let (stream, _) = std::net::TcpListener::bind("127.0.0.1:25565")
        .unwrap()
        .accept()
        .unwrap();
    let mut ps = PacketStream::new(std::io::BufReader::new(&stream), &stream);

    // TODO: multiple clients (increment cid)
    s.on_connect(todo_cid);

    loop {
        let packet = ps.next_packet();
        if let &InPacket::LoginStart { .. } = &packet {
            ps.send(OutPacket::LoginSuccess {
                uuid: 123,
                username: "foobar",
                props: Vec::new(),
            });
        }

        if let &InPacket::LoginAck = &packet {
            ps.send(OutPacket::FinishConfig);
        }

        if let &InPacket::FinishConfig = &packet {
            ps.send(OutPacket::LoginPlay {
                entity_id: 1,
                is_hardcore: false,
                dimension_names: vec!["foo:bar"],
                max_players: 456,
                view_distance: 111,
                simulation_distance: 222,
                reduced_debug_info: false,
                enable_respawn_screen: true,
                do_limited_crafting: false,
                dimension_type: "foo:baz",
                dimension_name: "foo:bar",
                hashed_seed: 999,
                game_mode: GameMode::Spectator,
                prev_game_mode: None,
                is_debug: false,
                is_superflat: false,
                death_info: None,
                portal_cooldown: 5,
            });
        }
        s.handle_packet(todo_cid, packet);
    }

    // TODO: multiple clients (increment cid)
    s.on_disconnect(todo_cid);
}
