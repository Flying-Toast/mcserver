use std::io::{Read, Write};

#[derive(Debug, Copy, Clone)]
pub enum HandshakeNextState {
    Status,
    Login,
}

#[derive(Debug, Copy, Clone)]
pub enum ChatMode {
    Enabled,
    CommandsOnly,
    Hidden,
}

#[derive(Debug, Copy, Clone)]
pub enum MainHand {
    Left,
    Right,
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
    LoginAck,
    PluginMessageConfig {
        // TODO: Identifier type?
        channel: String,
        data: Vec<u8>,
    },
    ClientInfoConfig {
        locale: String,
        view_distance: i8,
        chat_mode: ChatMode,
        chat_colors: bool,
        // TODO: make this a nice type
        displayed_skin_parts: u8,
        main_hand: MainHand,
        enable_text_filtering: bool,
        allow_server_listings: bool,
    },
    FinishConfig,
}

#[derive(Debug)]
pub struct LoginSuccessProp<'a> {
    pub name: &'a str,
    pub value: &'a str,
    pub signature: Option<&'a str>,
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum GameMode {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 3,
}

#[derive(Debug)]
pub struct Position {
    /// NOTE: this is actually only supposed to be 26 bits
    pub x: i32,
    /// NOTE: this is actually only supposed to be 26 bits
    pub z: i32,
    /// NOTE: this is actually only supposed to be 12 bits
    pub y: i16,
}

#[derive(Debug)]
pub struct DeathInfo<'a> {
    /// dimension the player died in
    // TODO: Identifier type?
    pub dimension: &'a str,
    pub location: Position,
}

#[derive(Debug)]
pub struct BitSet {
    longs: Vec<i64>,
}

impl BitSet {
    /// Creates a BitSet with the fewest # of longs needed to hold `n` bits. initializes all bits to 0
    pub fn with_num_bits(n: usize) -> Self {
        Self {
            longs: vec![0; n.div_ceil(64)],
        }
    }

    fn compute_idx(&self, bit_idx: usize) -> (usize, usize) {
        assert!(
            bit_idx < 64 * self.longs.len(),
            "bit index {bit_idx} out of range for nbits={}",
            64 * self.longs.len()
        );

        (bit_idx / 64, bit_idx % 64)
    }

    pub fn set(&mut self, bit_idx: usize) {
        let (long_idx, bit_idx) = self.compute_idx(bit_idx);
        self.longs[long_idx] |= 1 << bit_idx;
    }

    pub fn get(&self, bit_idx: usize) -> bool {
        let (long_idx, bit_idx) = self.compute_idx(bit_idx);

        (self.longs[long_idx] & (1 << bit_idx)) != 0
    }
}

// TODO: OutPacket trait, and make each outpacket variant its own type
#[derive(Debug)]
pub enum OutPacket<'a> {
    // TODO: implement full 'JSON Chat' structure
    DisconnectLogin {
        reason: &'a str,
    },
    LoginSuccess {
        uuid: u128,
        username: &'a str,
        //TODO: what are these props for?
        props: Vec<LoginSuccessProp<'a>>,
    },
    FinishConfig,
    LoginPlay {
        /// ID of the player entity
        entity_id: i32,
        is_hardcore: bool,
        // TODO: Identifier type?
        dimension_names: Vec<&'a str>,
        max_players: i64,
        view_distance: i64,
        simulation_distance: i64,
        reduced_debug_info: bool,
        enable_respawn_screen: bool,
        do_limited_crafting: bool,
        // TODO: Identifier type?
        dimension_type: &'a str,
        /// Name of the dimension the player is spawning into
        // TODO: Identifier type?
        dimension_name: &'a str,
        hashed_seed: i64,
        game_mode: GameMode,
        prev_game_mode: Option<GameMode>,
        is_debug: bool,
        is_superflat: bool,
        death_info: Option<DeathInfo<'a>>,
        portal_cooldown: i64,
    },
}

#[derive(Debug, Copy, Clone)]
enum State {
    Handshaking,
    Login,
    Config,
    Play,
}

// TODO: assert state is correct for each sent packet (e.g. LoginPlay cant be sent while in Config state)
#[derive(Debug)]
pub(crate) struct PacketStream<R: Read, W: Write> {
    r: R,
    w: W,
    state: State,
}

impl<R: Read, W: Write> PacketStream<R, W> {
    pub fn new(r: R, w: W) -> Self {
        Self {
            r,
            w,
            state: State::Handshaking,
        }
    }

    pub fn next_packet(&mut self) -> InPacket {
        let packet_len_field = self.read_varint();
        let (packid, packidnread) = self.read_varint_with_nread();
        let packet_tail_len = packet_len_field - packidnread;

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
            // LoginAck
            (0x03, State::Login) => {
                self.state = State::Config;

                InPacket::LoginAck
            }
            // PluginMessageConfig
            (0x01, State::Config) => {
                let (channel, strlen) = self.read_string_with_nread();
                let data_len = packet_tail_len - strlen;
                let mut data = vec![0; data_len.try_into().unwrap()];
                self.r.read_exact(&mut data).unwrap();

                InPacket::PluginMessageConfig { channel, data }
            }
            // ClientInfoConfig
            (0x00, State::Config) => {
                let locale = self.read_string();
                let view_distance = self.read_byte();
                let chat_mode = match self.read_varint() {
                    0 => ChatMode::Enabled,
                    1 => ChatMode::CommandsOnly,
                    2 => ChatMode::Hidden,
                    x => panic!("bad chat mode '{x}'"),
                };
                let chat_colors = self.read_bool();
                let displayed_skin_parts = self.read_ubyte();
                let main_hand = match self.read_varint() {
                    0 => MainHand::Left,
                    1 => MainHand::Right,
                    x => panic!("bad main hand '{x}'"),
                };
                let enable_text_filtering = self.read_bool();
                let allow_server_listings = self.read_bool();

                InPacket::ClientInfoConfig {
                    locale,
                    view_distance,
                    chat_mode,
                    chat_colors,
                    allow_server_listings,
                    enable_text_filtering,
                    displayed_skin_parts,
                    main_hand,
                }
            }
            (0x02, State::Config) => {
                self.state = State::Play;

                InPacket::FinishConfig
            }
            _ => panic!(
                "unknown packet '{:?}, 0x{packid:X}' (len = {packet_len_field})",
                self.state
            ),
        }
    }

    // TODO: buffer the entire packaet and only write it all at once
    pub fn send(&mut self, packet: OutPacket) {
        // TODO: reuse this vec. Or nicer way to do the length thing all together?
        let mut buf = Vec::new();
        {
            let buf = &mut buf;

            // hold a reference to the writer throughout the `match`
            // so that we don't accidentally write directly to self.w
            // instead of to `buf` :-)
            let prevent_oopsie_doopsie = &mut self.w;

            match packet {
                OutPacket::DisconnectLogin { reason } => {
                    // packet ID:
                    write_varint(buf, 0x00);

                    write!(buf, r#"{{text:"{reason}"}}"#).unwrap();
                }

                OutPacket::LoginSuccess {
                    uuid,
                    username,
                    props,
                } => {
                    // packet ID:
                    write_varint(buf, 0x02);

                    write_uuid(buf, uuid);
                    write_string(buf, username);
                    write_varint(buf, props.len().try_into().unwrap());
                    for p in props {
                        write_string(buf, p.name);
                        write_string(buf, p.value);
                        match p.signature {
                            Some(sig) => {
                                write_bool(buf, true);
                                write_string(buf, sig);
                            }
                            None => write_bool(buf, false),
                        }
                    }
                }

                OutPacket::LoginPlay {
                    entity_id,
                    is_hardcore,
                    dimension_names,
                    max_players,
                    view_distance,
                    simulation_distance,
                    reduced_debug_info,
                    enable_respawn_screen,
                    do_limited_crafting,
                    dimension_type,
                    dimension_name,
                    hashed_seed,
                    game_mode,
                    prev_game_mode,
                    is_debug,
                    is_superflat,
                    death_info,
                    portal_cooldown,
                } => {
                    // packet ID:
                    write_varint(buf, 0x29);

                    write_int(buf, entity_id);
                    write_bool(buf, is_hardcore);
                    write_varint(buf, dimension_names.len().try_into().unwrap());
                    for d in dimension_names.iter() {
                        write_string(buf, d);
                    }
                    write_varint(buf, max_players);
                    write_varint(buf, view_distance);
                    write_varint(buf, simulation_distance);
                    write_bool(buf, reduced_debug_info);
                    write_bool(buf, enable_respawn_screen);
                    write_bool(buf, do_limited_crafting);
                    write_string(buf, dimension_type);
                    write_string(buf, dimension_name);
                    write_long(buf, hashed_seed);
                    write_game_mode(buf, game_mode);
                    match prev_game_mode {
                        None => write_ibyte(buf, -1),
                        Some(gm) => write_game_mode(buf, gm),
                    }
                    write_bool(buf, is_debug);
                    write_bool(buf, is_superflat);
                    match death_info {
                        None => write_bool(buf, false),
                        Some(i) => {
                            write_bool(buf, true);
                            write_string(buf, i.dimension);
                            write_position(buf, &i.location);
                        }
                    }
                    write_varint(buf, portal_cooldown);
                }
                OutPacket::FinishConfig => {
                    // packet ID:
                    write_varint(buf, 0x02);
                }
            }

            let _ = prevent_oopsie_doopsie;
        }
        write_varint(&mut self.w, buf.len().try_into().unwrap());
        self.w.write(&buf).unwrap();
    }

    fn read_varint(&mut self) -> i64 {
        self.read_varint_with_nread().0
    }

    // returns the varint and how many bytes were read for it.
    // returns (varint, nread).
    fn read_varint_with_nread(&mut self) -> (i64, i64) {
        let mut ret = 0;
        let mut shift = 0;
        let mut nread = 0;

        let mut b = [0];
        loop {
            self.r.read_exact(&mut b).unwrap();
            nread += 1;
            let cur = b[0];
            ret |= ((cur & 0b01111111) as i64) << shift;
            shift += 7;
            if cur & (1 << 7) == 0 {
                break;
            }
        }

        (ret, nread)
    }

    // returns the read string and how many bytes were read to deserialize the string.
    // (because of Java's stupid "Modified UTF-8" the # of bytes read might differ from string.len().
    fn read_string_with_nread(&mut self) -> (String, i64) {
        let (len, lennread) = self.read_varint_with_nread();
        let mut vs = vec![0; len.try_into().unwrap()];
        self.r.read_exact(&mut vs).unwrap();
        // TODO: convert from Java's "Modified UTF-8" :(
        (String::from_utf8(vs).unwrap(), len + lennread)
    }

    fn read_string(&mut self) -> String {
        self.read_string_with_nread().0
    }

    fn read_ushort(&mut self) -> u16 {
        let mut b = [0, 0];
        self.r.read_exact(&mut b).unwrap();
        u16::from_be_bytes(b)
    }

    fn read_byte(&mut self) -> i8 {
        let mut b = [0];
        self.r.read_exact(&mut b).unwrap();
        i8::from_be_bytes(b)
    }

    fn read_ubyte(&mut self) -> u8 {
        let mut b = [0];
        self.r.read_exact(&mut b).unwrap();
        b[0]
    }

    fn read_bool(&mut self) -> bool {
        match self.read_ubyte() {
            0 => false,
            1 => true,
            _ => panic!("bad bool"),
        }
    }

    fn read_uuid(&mut self) -> u128 {
        let mut b = [0; 16];
        self.r.read_exact(&mut b).unwrap();
        u128::from_be_bytes(b)
    }
}

// TODO: is this really correct? negative numbers always send 64 bits?
fn write_varint<W: Write>(w: &mut W, int: i64) {
    let seg_bits = 0b01111111;
    let mut int = u64::from_ne_bytes(int.to_ne_bytes());

    loop {
        if int & !seg_bits == 0 {
            write_ubyte(w, (int & 0xFF).try_into().unwrap());
            break;
        }

        write_ubyte(w, ((int & seg_bits) | (1 << 7)).try_into().unwrap());
        int >>= 7;
    }
}

fn write_ubyte<W: Write>(w: &mut W, byte: u8) {
    w.write(&[byte]).unwrap();
}

fn write_ibyte<W: Write>(w: &mut W, byte: i8) {
    w.write(&byte.to_be_bytes()).unwrap();
}

fn write_uuid<W: Write>(w: &mut W, uuid: u128) {
    w.write(&uuid.to_be_bytes()).unwrap();
}

fn write_string<W: Write>(w: &mut W, s: &str) {
    write_varint(w, s.len().try_into().unwrap());
    // TODO: java's dumbass "Modified UTF-8" again
    w.write(s.as_bytes()).unwrap();
}

fn write_bool<W: Write>(w: &mut W, b: bool) {
    write_ubyte(w, b as u8);
}

fn write_int<W: Write>(w: &mut W, int: i32) {
    w.write(&int.to_be_bytes()).unwrap();
}

fn write_long<W: Write>(w: &mut W, long: i64) {
    w.write(&long.to_be_bytes()).unwrap();
}

fn write_game_mode<W: Write>(w: &mut W, gm: GameMode) {
    write_ubyte(w, gm as u8);
}

fn write_position<W: Write>(w: &mut W, p: &Position) {
    let mask_26bits: i64 = 0x3FFFFFF;
    let mask_12bits: i64 = 0xFFF;

    let x = p.x as i64;
    let z = p.z as i64;
    let y = p.y as i64;

    let mut packed: i64 = 0;

    assert!(x == x & mask_26bits, "Position.x is bigger than 26 bits");
    assert!(z == z & mask_26bits, "Position.z is bigger than 26 bits");
    assert!(y == y & mask_12bits, "Position.y is bigger than 12 bits");

    packed |= (x & mask_26bits) << 38;
    packed |= (z & mask_26bits) << 12;
    packed |= y & mask_12bits;

    w.write(&packed.to_be_bytes()).unwrap();
}

fn write_bitset<W: Write>(w: &mut W, bs: &BitSet) {
    write_varint(w, bs.longs.len().try_into().unwrap());
    for l in bs.longs.iter().copied() {
        write_long(w, l);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitset() {
        let mut bs = BitSet::with_num_bits(1);
        assert!(bs.longs.len() == 1);

        let set_bit_idxs = [12, 1, 0, 4];
        for i in set_bit_idxs.iter() {
            bs.set(*i);
        }

        for i in 0..64 {
            if set_bit_idxs.contains(&i) {
                assert!(bs.get(i), "Unexpected bit {i} = 0");
            } else {
                assert!(!bs.get(i), "Unexpected bit {i} = 1");
            }
        }
    }
}
