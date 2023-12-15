use crate::*;
use std::io::{Read, Write};

#[derive(Debug, Copy, Clone)]
pub enum HandshakeNextState {
    Status,
    Login,
}

#[derive(Debug)]
pub enum InPacket {
    Handshake {
        protocol_version: i64,
        server_addr: String,
        server_port: u16,
        next_state: HandshakeNextState,
    },
    LoginStart {
        name: String,
        player_uuid: u128,
    },
}

#[derive(Debug)]
pub enum OutPacket {
    Disconnect { reason: &'static str },
}

impl OutPacket {
    // TODO: buffer the whole packet into a single write
    pub(crate) fn serialize_to<W: Write>(&self, mut w: W) {
        match self {
            Self::Disconnect { reason } => {
                write!(w, r#"{{text:"{}",bold:true}}"#, reason).unwrap();
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum State {
    Handshaking,
    Login,
}

#[derive(Debug)]
pub(crate) struct PacketReader<R: Read> {
    r: R,
    state: State,
}

impl<R: Read> PacketReader<R> {
    pub fn new(r: R) -> Self {
        Self {
            r,
            state: State::Handshaking,
        }
    }

    pub fn next_packet(&mut self) -> InPacket {
        let _len = self.read_varint();
        let packid = self.read_varint();

        match (packid, self.state) {
            // Handshake
            (0x00, State::Handshaking) => {
                let protocol_version = self.read_varint();
                let server_addr = self.read_string();
                let server_port = self.read_ushort();
                let next_state = match self.read_varint() {
                    1 => HandshakeNextState::Status,
                    2 => HandshakeNextState::Login,
                    x => panic!("bad next state {x}"),
                };
                self.state = State::Login;

                InPacket::Handshake {
                    protocol_version,
                    server_addr,
                    server_port,
                    next_state,
                }
            }
            // Login Start
            (0x00, State::Login) => {
                let name = self.read_string();
                let player_uuid = self.read_uuid();
                InPacket::LoginStart { name, player_uuid }
            }
            _ => panic!("unknown packet id {packid}"),
        }
    }

    fn read_varint(&mut self) -> i64 {
        let mut ret = 0;
        let mut shift = 0;

        let mut b = [0];
        loop {
            self.r.read_exact(&mut b).unwrap();
            let cur = b[0];
            ret |= ((cur & 0b01111111) as i64) << shift;
            shift += 7;
            if cur & (1 << 7) == 0 {
                break;
            }
        }

        ret
    }

    fn read_string(&mut self) -> String {
        let len: usize = self.read_varint().try_into().unwrap();
        let mut vs = vec![0; len];
        self.r.read_exact(&mut vs).unwrap();
        // TODO: convert from Java's "Modified UTF-8" :(
        String::from_utf8(vs).unwrap()
    }

    fn read_ushort(&mut self) -> u16 {
        let mut b = [0, 0];
        self.r.read_exact(&mut b).unwrap();
        u16::from_be_bytes(b)
    }

    fn read_uuid(&mut self) -> u128 {
        let mut b = [0; 16];
        self.r.read_exact(&mut b).unwrap();
        u128::from_be_bytes(b)
    }
}
