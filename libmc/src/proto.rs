use crate::*;
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

#[derive(Debug)]
pub struct BlockEntity<'a> {
    /// Valid values: 0-15
    pub x: u8,
    pub z: u8,
    pub y: i16,
    /// type
    pub tipe: i64,
    pub data: CompoundNbt<'a>,
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
    ChunkDataAndUpdateLight {
        chunk_x: i32,
        chunk_z: i32,
        heightmaps: CompoundNbt<'a>,
        data: &'a [i8],
        block_entities: &'a [BlockEntity<'a>],
        sky_light_mask: BitSet,
        block_light_mask: BitSet,
        empty_sky_light_mask: BitSet,
        empty_block_light_mask: BitSet,
        sky_light_arrays: &'a [[i8; 2048]],
        block_light_arrays: &'a [[i8; 2048]],
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
        let packet_len_field = read_varint(&mut self.r);
        let (packid, packidnread) = read_varint_with_nread(&mut self.r);
        let packet_tail_len = packet_len_field - packidnread;

        match (packid, self.state) {
            // Handshake
            (0x00, State::Handshaking) => {
                let protocol_version = read_varint(&mut self.r);
                let server_addr = read_varint_string(&mut self.r);
                let server_port = read_ushort(&mut self.r);
                let next_state = match read_varint(&mut self.r) {
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
                let name = read_varint_string(&mut self.r);
                let player_uuid = read_uuid(&mut self.r);
                InPacket::LoginStart { name, player_uuid }
            }
            // LoginAck
            (0x03, State::Login) => {
                self.state = State::Config;

                InPacket::LoginAck
            }
            // PluginMessageConfig
            (0x01, State::Config) => {
                let (channel, strlen) = read_varint_string_with_nread(&mut self.r);
                let data_len = packet_tail_len - strlen;
                let mut data = vec![0; data_len.try_into().unwrap()];
                self.r.read_exact(&mut data).unwrap();

                InPacket::PluginMessageConfig { channel, data }
            }
            // ClientInfoConfig
            (0x00, State::Config) => {
                let locale = read_varint_string(&mut self.r);
                let view_distance = read_byte(&mut self.r);
                let chat_mode = match read_varint(&mut self.r) {
                    0 => ChatMode::Enabled,
                    1 => ChatMode::CommandsOnly,
                    2 => ChatMode::Hidden,
                    x => panic!("bad chat mode '{x}'"),
                };
                let chat_colors = read_bool(&mut self.r);
                let displayed_skin_parts = read_ubyte(&mut self.r);
                let main_hand = match read_varint(&mut self.r) {
                    0 => MainHand::Left,
                    1 => MainHand::Right,
                    x => panic!("bad main hand '{x}'"),
                };
                let enable_text_filtering = read_bool(&mut self.r);
                let allow_server_listings = read_bool(&mut self.r);

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
                OutPacket::ChunkDataAndUpdateLight {
                    chunk_x,
                    chunk_z,
                    heightmaps,
                    data,
                    block_entities,
                    sky_light_mask,
                    block_light_mask,
                    empty_sky_light_mask,
                    empty_block_light_mask,
                    sky_light_arrays,
                    block_light_arrays,
                } => {
                    // packet ID:
                    write_varint(buf, 0x25);

                    write_int(buf, chunk_x);
                    write_int(buf, chunk_z);
                    write_compound_nbt(buf, &heightmaps);
                    write_varint(buf, data.len().try_into().unwrap());
                    for x in data.iter().copied() {
                        write_ibyte(buf, x);
                    }
                    write_varint(buf, block_entities.len().try_into().unwrap());
                    for bent in block_entities.iter() {
                        write_block_entity(buf, bent);
                    }
                    write_bitset(buf, &sky_light_mask);
                    write_bitset(buf, &block_light_mask);
                    write_bitset(buf, &empty_sky_light_mask);
                    write_bitset(buf, &empty_block_light_mask);
                    write_varint(buf, sky_light_arrays.len().try_into().unwrap());
                    for arr in sky_light_arrays.iter() {
                        write_varint(buf, 2048);
                        for b in arr.iter().copied() {
                            write_ibyte(buf, b);
                        }
                    }
                    write_varint(buf, block_light_arrays.len().try_into().unwrap());
                    for arr in block_light_arrays.iter() {
                        write_varint(buf, 2048);
                        for b in arr.iter().copied() {
                            write_ibyte(buf, b);
                        }
                    }
                }
            }

            let _ = prevent_oopsie_doopsie;
        }
        write_varint(&mut self.w, buf.len().try_into().unwrap());
        self.w.write(&buf).unwrap();
    }
}

pub(crate) fn read_varint<R: Read>(r: &mut R) -> i64 {
    read_varint_with_nread(r).0
}

// returns the varint and how many bytes were read for it.
// returns (varint, nread).
pub(crate) fn read_varint_with_nread<R: Read>(r: &mut R) -> (i64, i64) {
    let mut ret = 0;
    let mut shift = 0;
    let mut nread = 0;

    let mut b = [0];
    loop {
        r.read_exact(&mut b).unwrap();
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

/// Reads a string prefixed by its length as a varint.
/// Returns the read string and how many bytes were read to deserialize the string.
/// (because of Java's stupid "Modified UTF-8" the # of bytes read might differ from string.len().
pub(crate) fn read_varint_string_with_nread<R: Read>(r: &mut R) -> (String, i64) {
    let (len, lennread) = read_varint_with_nread(r);
    let mut vs = vec![0; len.try_into().unwrap()];
    r.read_exact(&mut vs).unwrap();
    // TODO: convert from Java's "Modified UTF-8" :(
    (String::from_utf8(vs).unwrap(), len + lennread)
}

pub(crate) fn read_ushort_string<R: Read>(r: &mut R) -> String {
    let len = read_ushort(r);
    let mut vs = vec![0; len.try_into().unwrap()];
    r.read_exact(&mut vs).unwrap();
    // TODO: convert from Java's "Modified UTF-8" :(
    String::from_utf8(vs).unwrap()
}

pub(crate) fn write_ushort_string<W: Write>(w: &mut W, s: &str) {
    write_ushort(w, s.len().try_into().unwrap());
    // TODO: convert to java "Modified UTF-8"
    w.write(s.as_bytes()).unwrap();
}

pub(crate) fn read_varint_string<R: Read>(r: &mut R) -> String {
    read_varint_string_with_nread(r).0
}

pub(crate) fn read_short<R: Read>(r: &mut R) -> i16 {
    let mut b = [0, 0];
    r.read_exact(&mut b).unwrap();
    i16::from_be_bytes(b)
}

pub(crate) fn read_ushort<R: Read>(r: &mut R) -> u16 {
    let mut b = [0, 0];
    r.read_exact(&mut b).unwrap();
    u16::from_be_bytes(b)
}

pub(crate) fn read_int<R: Read>(r: &mut R) -> i32 {
    let mut b = [0; 4];
    r.read_exact(&mut b).unwrap();
    i32::from_be_bytes(b)
}

pub(crate) fn read_long<R: Read>(r: &mut R) -> i64 {
    let mut b = [0; 8];
    r.read_exact(&mut b).unwrap();
    i64::from_be_bytes(b)
}

pub(crate) fn read_byte<R: Read>(r: &mut R) -> i8 {
    let mut b = [0];
    r.read_exact(&mut b).unwrap();
    i8::from_be_bytes(b)
}

pub(crate) fn read_ubyte<R: Read>(r: &mut R) -> u8 {
    let mut b = [0];
    r.read_exact(&mut b).unwrap();
    b[0]
}

pub(crate) fn read_float<R: Read>(r: &mut R) -> f32 {
    let mut b = [0; 4];
    r.read_exact(&mut b).unwrap();
    f32::from_be_bytes(b)
}

pub(crate) fn read_double<R: Read>(r: &mut R) -> f64 {
    let mut b = [0; 8];
    r.read_exact(&mut b).unwrap();
    f64::from_be_bytes(b)
}

pub(crate) fn read_bool<R: Read>(r: &mut R) -> bool {
    match read_ubyte(r) {
        0 => false,
        1 => true,
        _ => panic!("bad bool"),
    }
}

pub(crate) fn read_uuid<R: Read>(r: &mut R) -> u128 {
    let mut b = [0; 16];
    r.read_exact(&mut b).unwrap();
    u128::from_be_bytes(b)
}

// TODO: is this really correct? negative numbers always send 64 bits?
pub(crate) fn write_varint<W: Write>(w: &mut W, int: i64) {
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

pub(crate) fn write_ubyte<W: Write>(w: &mut W, byte: u8) {
    w.write(&[byte]).unwrap();
}

pub(crate) fn write_ibyte<W: Write>(w: &mut W, byte: i8) {
    w.write(&byte.to_be_bytes()).unwrap();
}

pub(crate) fn write_short<W: Write>(w: &mut W, short: i16) {
    w.write(&short.to_be_bytes()).unwrap();
}

pub(crate) fn write_ushort<W: Write>(w: &mut W, ushort: u16) {
    w.write(&ushort.to_be_bytes()).unwrap();
}

pub(crate) fn write_uuid<W: Write>(w: &mut W, uuid: u128) {
    w.write(&uuid.to_be_bytes()).unwrap();
}

pub(crate) fn write_string<W: Write>(w: &mut W, s: &str) {
    write_varint(w, s.len().try_into().unwrap());
    // TODO: java's dumbass "Modified UTF-8" again
    w.write(s.as_bytes()).unwrap();
}

pub(crate) fn write_bool<W: Write>(w: &mut W, b: bool) {
    write_ubyte(w, b as u8);
}

pub(crate) fn write_int<W: Write>(w: &mut W, int: i32) {
    w.write(&int.to_be_bytes()).unwrap();
}

pub(crate) fn write_long<W: Write>(w: &mut W, long: i64) {
    w.write(&long.to_be_bytes()).unwrap();
}

pub(crate) fn write_float<W: Write>(w: &mut W, x: f32) {
    w.write(&x.to_be_bytes()).unwrap();
}

pub(crate) fn write_double<W: Write>(w: &mut W, x: f64) {
    w.write(&x.to_be_bytes()).unwrap();
}

pub(crate) fn write_game_mode<W: Write>(w: &mut W, gm: GameMode) {
    write_ubyte(w, gm as u8);
}

pub(crate) fn write_position<W: Write>(w: &mut W, p: &Position) {
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

pub(crate) fn write_bitset<W: Write>(w: &mut W, bs: &BitSet) {
    write_varint(w, bs.longs.len().try_into().unwrap());
    for l in bs.longs.iter().copied() {
        write_long(w, l);
    }
}

pub(crate) fn write_block_entity<W: Write>(w: &mut W, bent: &BlockEntity<'_>) {
    write_ibyte(w, ((bent.x as i8 & 15) << 4) | (bent.z as i8 & 15));
    write_short(w, bent.y);
    write_varint(w, bent.tipe);
    write_compound_nbt(w, &bent.data);
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
