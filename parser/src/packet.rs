use log::warn;
use nom::{
    bytes::complete::take, multi::count, number::complete::be_u32, number::complete::be_u8,
    number::complete::le_f32, number::complete::le_u32,
};
use serde_derive::Serialize;
use std::convert::TryInto;

use crate::error::*;
use crate::parse_77::*;

#[derive(Debug, Serialize)]
pub struct PositionPacket {
    pub pid: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_x: u32,
    pub rot_y: u32,
    pub rot_z: u32,
    pub a: f32, // These three appear to be velocity in x,y,z (perhaps local? Forward/back velocity and side-to-side drift?)
    pub b: f32,
    pub c: f32,
    pub extra: u8,
}

#[derive(Debug, Serialize)]
pub struct EntityPacket<'a> {
    pub supertype: u32,
    pub entity_id: u32,
    pub subtype: u32,
    pub payload: &'a [u8],
}

#[derive(Debug, Serialize)]
pub struct ChatPacket<'a> {
    pub entity_id: u32, // TODO: Is entity ID different than sender ID?
    pub sender_id: u32,
    pub audience: &'a str,
    pub message: &'a str,
}

#[derive(Debug, Serialize)]
pub struct TimingPacket {
    pub time: u32,
}

#[derive(Debug, Serialize)]
pub struct Type24Packet {
    pub f0: f32,
    pub f1: f32,
    pub f2: f32,
    pub f3: f32,
    pub f4: f32,
    pub f5: f32,
    pub f6: f32,
    pub f7: f32,
    pub f8: f32,
    pub f9: f32,
    pub f10: f32,
    pub f11: f32,
    pub f12: f32,
    pub f13: f32,
}

/// Note that this packet frequently appears twice - it appears that it
/// describes both the player's boat location/orientation as well as the
/// camera orientation. When the camera is attached to an object, the ID of
/// that object will be given in the parent_id field.
#[derive(Debug, Serialize)]
pub struct PlayerOrientationPacket {
    pub pid: u32,
    pub parent_id: u32,
    pub x: f32,

    /// I'm not 100% sure about this field
    pub y: f32,

    pub z: f32,

    /// Radians, 0 is North and positive numbers are clockwise
    /// e.g. pi/2 is due East, -pi/2 is due West, and +/-pi is due South.
    pub heading: f32,

    pub f4: f32,
    pub f5: f32,
}

#[derive(Debug, Serialize)]
pub struct ArtilleryHitPacket<'a> {
    pub subject: u32, // A player ID
    pub is_incoming: bool,
    pub is_he: bool,
    pub is_secondary: bool,
    pub damage: u32,
    pub incapacitations: Vec<u32>,
    pub bitmask0: u32,
    pub bitmask1: u32,
    pub bitmask2: u32,
    pub bitmask3: u32,
    pub bitmask4: u32,
    pub bitmask5: u32,
    pub raw: &'a [u8],
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum VoiceLine {
    IntelRequired,
    FairWinds,
    Wilco,
    Negative,
    WellDone,
    Curses,
    UsingRadar,
    UsingHydroSearch,
    DefendTheBase, // TODO: ...except when it's "thank you"?
    SetSmokeScreen,
    ProvideAntiAircraft,
    RequestingSupport(Option<u32>),
    Retreat(Option<u32>),

    /// Fields are (letter,number) and zero-indexed. e.g. F2 is (5,1)
    AttentionToSquare((u32, u32)),

    /// Field is the ID of the target
    ConcentrateFire(u32),
}

#[derive(Debug, Serialize)]
pub struct VoiceLinePacket {
    pub sender: u32,

    /// Voice lines are either to everyone or to the team
    pub is_global: bool,

    pub message: VoiceLine,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize)]
pub enum DeathCause {
    Secondaries,
    Artillery,
    Fire,
    Flooding,
    Torpedo,
    DiveBomber,
    AerialRocket,
    AerialTorpedo,
    Detonation,
    Ramming,
}

#[derive(Debug, Serialize)]
pub struct ShipDestroyedPacket {
    pub victim: u32,
    pub killer: u32,
    pub death_cause: DeathCause,
    pub unknown: u32,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize)]
pub enum Banner {
    PlaneShotDown,
    Incapacitation,
    SetFire,
    Citadel,
    SecondaryHit,
    OverPenetration,
    Penetration,
    NonPenetration,
    Ricochet,
    TorpedoProtectionHit,
    Captured,
    AssistedInCapture,
    Spotted,
    Destroyed,
    TorpedoHit,
    Defended,
    Flooding,
    DiveBombPenetration,
    RocketPenetration,
    RocketNonPenetration,
    RocketTorpedoProtectionHit,
    ShotDownByAircraft,
}

#[derive(Debug, Serialize)]
pub struct DamageReceivedPacket {
    recipient: u32,
    damage: Vec<(u32, f32)>,
}

#[derive(Debug, Serialize)]
pub struct InvalidPacket<'a> {
    message: String,
    raw: &'a [u8],
}

#[derive(Debug, Serialize)]
pub enum PacketType<'a> {
    Position(PositionPacket),
    Entity(EntityPacket<'a>), // 0x7 and 0x8 are known to be of this type
    Chat(ChatPacket<'a>),
    Timing(TimingPacket),
    ArtilleryHit(ArtilleryHitPacket<'a>),
    Banner(Banner),
    DamageReceived(DamageReceivedPacket),
    Type24(Type24Packet),
    PlayerOrientation(PlayerOrientationPacket),
    Type8_79(Vec<(u32, u32)>),
    Setup(SetupPacket),
    ShipDestroyed(ShipDestroyedPacket),
    VoiceLine(VoiceLinePacket),
    Unknown(&'a [u8]),

    /// These are packets which we thought we understood, but couldn't parse
    Invalid(InvalidPacket<'a>),
}

#[derive(Debug, Serialize)]
pub struct Packet<'a> {
    pub packet_size: u32,
    pub packet_type: u32,
    pub clock: f32,
    pub payload: PacketType<'a>,
    pub raw: &'a [u8],
}

fn parse_voiceline_packet(
    _entity_id: u32,
    _supertype: u32,
    _subtype: u32,
    payload: &[u8],
) -> IResult<&[u8], PacketType> {
    let (i, audience) = be_u8(payload)?;
    //assert!(audience == 0); // What do the other audiences mean?
    let (i, sender) = le_u32(i)?;
    let (i, line) = be_u8(i)?;
    let (i, a) = le_u32(i)?;
    let (i, b) = le_u32(i)?;
    let (i, c) = le_u32(i)?;
    let is_global = match audience {
        0 => false,
        1 => true,
        _ => {
            panic!(format!(
                "Got unknown audience {} sender=0x{:x} line={} a={:x} b={:x} c={:x}",
                audience, sender, line, a, b, c
            ));
        }
    };
    let message = match line {
        1 => VoiceLine::AttentionToSquare((a, b)),
        2 => VoiceLine::ConcentrateFire(b),
        3 => VoiceLine::RequestingSupport(None),
        5 => VoiceLine::Wilco,
        6 => VoiceLine::Negative,
        7 => VoiceLine::WellDone, // TODO: Find the corresponding field
        8 => VoiceLine::FairWinds,
        9 => VoiceLine::Curses,
        10 => VoiceLine::DefendTheBase,
        11 => VoiceLine::ProvideAntiAircraft,
        12 => VoiceLine::Retreat(if b != 0 { Some(b) } else { None }),
        13 => VoiceLine::IntelRequired,
        14 => VoiceLine::SetSmokeScreen,
        15 => VoiceLine::UsingRadar,
        16 => VoiceLine::UsingHydroSearch,
        _ => {
            panic!(format!(
                "Unknown voice line {} a={:x} b={:x} c={:x}!",
                line, a, b, c
            ));
        }
    };
    Ok((
        i,
        PacketType::VoiceLine(VoiceLinePacket {
            sender,
            is_global,
            message,
        }),
    ))
}

fn parse_ship_destroyed_packet(
    _entity_id: u32,
    _supertype: u32,
    _subtype: u32,
    payload: &[u8],
) -> IResult<&[u8], PacketType> {
    let (i, victim) = le_u32(payload)?;
    let (i, killer) = le_u32(i)?;
    let (i, unknown) = le_u32(i)?;
    let death_cause = match unknown {
        2 => DeathCause::Secondaries,
        3 => DeathCause::Torpedo,
        4 => DeathCause::DiveBomber,
        5 => DeathCause::AerialTorpedo,
        6 => DeathCause::Fire,
        7 => DeathCause::Ramming,
        9 => DeathCause::Flooding,
        14 => DeathCause::AerialRocket,
        15 => DeathCause::Detonation,
        17 => DeathCause::Artillery,
        18 => DeathCause::Artillery,
        19 => DeathCause::Artillery,
        _ => {
            panic!(format!("Found unknown death_cause {}", unknown));
        }
    };
    Ok((
        i,
        PacketType::ShipDestroyed(ShipDestroyedPacket {
            victim,
            killer,
            death_cause,
            unknown,
        }),
    ))
}

fn parse_player_orientation_packet(i: &[u8]) -> IResult<&[u8], PacketType> {
    assert!(i.len() == 0x20);
    let (i, pid) = le_u32(i)?;
    let (i, parent_id) = le_u32(i)?;
    let (i, x) = le_f32(i)?;
    let (i, y) = le_f32(i)?;
    let (i, z) = le_f32(i)?;
    let (i, heading) = le_f32(i)?;
    let (i, f4) = le_f32(i)?;
    let (i, f5) = le_f32(i)?;
    Ok((
        i,
        PacketType::PlayerOrientation(PlayerOrientationPacket {
            pid,
            parent_id,
            x,
            y,
            z,
            heading,
            f4,
            f5,
        }),
    ))
}

/// Perhaps camera data or something?
fn parse_type_24_packet(i: &[u8]) -> IResult<&[u8], PacketType> {
    assert!(i.len() == 56);
    let (i, f0) = le_f32(i)?;
    let (i, f1) = le_f32(i)?;
    let (i, f2) = le_f32(i)?;
    let (i, f3) = le_f32(i)?;
    let (i, f4) = le_f32(i)?;
    let (i, f5) = le_f32(i)?;
    let (i, f6) = le_f32(i)?;
    let (i, f7) = le_f32(i)?;
    let (i, f8) = le_f32(i)?;
    let (i, f9) = le_f32(i)?;
    let (i, f10) = le_f32(i)?;
    let (i, f11) = le_f32(i)?;
    let (i, f12) = le_f32(i)?;
    let (i, f13) = le_f32(i)?;
    Ok((
        i,
        PacketType::Type24(Type24Packet {
            f0,
            f1,
            f2,
            f3,
            f4,
            f5,
            f6,
            f7,
            f8,
            f9,
            f10,
            f11,
            f12,
            f13,
        }),
    ))
}

/// Note! There are actually two types of timing packets, which always seem to
/// mirror each other. ID N is immediately followed by ID N+1 w/ matching
/// counter. The counter seems to increment ~ once per ms
fn parse_timing_packet(
    _entity_id: u32,
    supertype: u32,
    subtype: u32,
    i: &[u8],
) -> IResult<&[u8], PacketType> {
    let raw = i;
    let (i, time) = le_u32(i)?;
    let (i, zero) = le_u32(i)?;
    // TODO: Re-enable
    //assert!(zero == 0); // What does this field mean?
    if zero != 0 {
        return Err(failure_from_kind(ErrorKind::UnableToProcessPacket {
            supertype,
            subtype,
            reason: format!(
                "Expected second integer to be zero in timing packet, was {}",
                zero
            ),
            packet: raw.to_vec(),
        }));
    }
    Ok((i, PacketType::Timing(TimingPacket { time })))
}

fn parse_chat_packet(
    entity_id: u32,
    _supertype: u32,
    _subtype: u32,
    i: &[u8],
) -> IResult<&[u8], PacketType> {
    let (i, sender) = le_u32(i)?;
    let (i, audience_len) = be_u8(i)?;
    let (i, audience) = take(audience_len)(i)?;
    let (i, message_len) = be_u8(i)?;
    let (i, message) = take(message_len)(i)?;
    Ok((
        i,
        PacketType::Chat(ChatPacket {
            entity_id: entity_id,
            sender_id: sender,
            audience: std::str::from_utf8(audience).unwrap(),
            message: std::str::from_utf8(message).unwrap(),
        }),
    ))
}

fn parse_8_79_subobject(i: &[u8]) -> IResult<&[u8], (u32, u32)> {
    let (i, pid) = le_u32(i)?;
    let (i, data) = le_u32(i)?;
    Ok((i, (pid, data)))
}

fn parse_8_79_packet(
    _entity_id: u32,
    _supertype: u32,
    _subtype: u32,
    i: &[u8],
) -> IResult<&[u8], PacketType> {
    //hexdump::hexdump(i);
    let (i, num_objects) = be_u8(i)?;
    //println!("Found {} objects", num_objects);
    let (i, objects) = count(parse_8_79_subobject, num_objects.try_into().unwrap())(i)?;
    let (i, b) = be_u8(i)?;
    assert!(b == 0); // What does this field do?
    assert!(i.len() == 0);
    Ok((i, PacketType::Type8_79(objects)))
}

fn parse_artillery_hit_packet(
    _entity_id: u32,
    supertype: u32,
    subtype: u32,
    i: &[u8],
) -> IResult<&[u8], PacketType> {
    let raw = i;
    let (i, bitmask0) = le_u32(i)?;
    let (i, bitmask1) = le_u32(i)?;
    let (i, bitmask2) = le_u32(i)?;
    let (i, subject) = le_u32(i)?;
    let (i, damage) = le_u32(i)?;
    let (i, bitmask3) = le_u32(i)?;
    let (i, bitmask4) = le_u32(i)?;
    let (i, bitmask5) = le_u32(i)?;
    let (i, incapacitation_count) = be_u8(i)?;
    let (i, incapacitations) = count(le_u32, incapacitation_count.try_into().unwrap())(i)?;
    /*let (i, incapacitations) = match incapacitation_count {
        0 => { (i, None) }
        1 => {
            let (a, b) = le_u32(i)?;
            (i, Some(b))
        }
        _ => {
            hexdump::hexdump(raw);
            panic!("Got unexpected incapacitation count {}!", incapacitation_count);
        }
    };*/
    let is_secondary = match (bitmask5 & 0xFF000000) >> 24 {
        0 => false,
        0xff => true,
        _ => {
            return Err(failure_from_kind(ErrorKind::UnableToProcessPacket {
                supertype,
                subtype,
                reason: format!(
                    "Got unknown value 0x{:x} for secondary bitfield in artillery packet",
                    bitmask5
                ),
                packet: raw.to_vec(),
            }));
        }
    };
    let is_he = false; //(bitmask0 & (1 << 22)) != 0;
    let is_incoming = (bitmask1 & (1 << 0)) != 0;
    assert!(i.len() == 0);
    Ok((
        i,
        PacketType::ArtilleryHit(ArtilleryHitPacket {
            subject,
            is_incoming,
            is_he,
            is_secondary,
            damage,
            incapacitations,
            bitmask0,
            bitmask1,
            bitmask2,
            bitmask3,
            bitmask4,
            bitmask5,
            raw,
        }),
    ))
}

fn parse_banner_packet(
    _entity_id: u32,
    supertype: u32,
    subtype: u32,
    i: &[u8],
) -> IResult<&[u8], PacketType> {
    let raw = i;
    let (i, banner) = be_u8(i)?;
    let banner = match banner {
        1 => Banner::TorpedoHit,
        3 => Banner::PlaneShotDown,
        4 => Banner::Incapacitation,
        5 => Banner::Destroyed,
        6 => Banner::SetFire,
        7 => Banner::Flooding,
        8 => Banner::Citadel,
        9 => Banner::Defended,
        10 => Banner::Captured,
        11 => Banner::AssistedInCapture,
        13 => Banner::SecondaryHit,
        14 => Banner::OverPenetration,
        15 => Banner::Penetration,
        16 => Banner::NonPenetration,
        17 => Banner::Ricochet,
        19 => Banner::Spotted,
        21 => Banner::DiveBombPenetration,
        25 => Banner::RocketPenetration,
        26 => Banner::RocketNonPenetration,
        27 => Banner::ShotDownByAircraft,
        28 => Banner::TorpedoProtectionHit,
        30 => Banner::RocketTorpedoProtectionHit,
        _ => {
            return Err(failure_from_kind(ErrorKind::UnableToProcessPacket {
                supertype,
                subtype,
                reason: format!("Got unknown banner type 0x{:x}", banner),
                packet: raw.to_vec(),
            }));
        }
    };
    Ok((i, PacketType::Banner(banner)))
}

fn parse_damage_received_part(i: &[u8]) -> IResult<&[u8], (u32, f32)> {
    let (i, pid) = le_u32(i)?;
    let (i, damage) = le_f32(i)?;
    Ok((i, (pid, damage)))
}

fn parse_damage_received_packet(
    entity_id: u32,
    supertype: u32,
    subtype: u32,
    i: &[u8],
) -> IResult<&[u8], PacketType> {
    let raw = i;
    let (i, cnt) = be_u8(i)?;
    if cnt == 0 {
        // TODO: It's not clear what's actually happening here
        // This behaviour is ostensible new to 0.9.5(.1?)
        //hexdump::hexdump(i);
        assert!(i.len() == 5);
        return Ok((
            &[],
            PacketType::Entity(EntityPacket {
                supertype,
                entity_id,
                subtype,
                payload: i,
            }),
        ));
    }
    if i.len() != 8 * cnt as usize {
        //println!("Unclear damage recv'd packet: cnt={}", cnt);
        //hexdump::hexdump(i);
        //panic!();
        return Ok((
            &[],
            PacketType::Invalid(InvalidPacket {
                message: format!("Unclear damage recv'd packet: cnt={}", cnt),
                raw: raw,
            }),
        ));
    }
    let (i, data) = count(parse_damage_received_part, cnt.try_into().unwrap())(i)?;
    assert!(i.len() == 0);
    Ok((
        i,
        PacketType::DamageReceived(DamageReceivedPacket {
            recipient: entity_id,
            damage: data,
        }),
    ))
}

fn parse_setup_packet(
    _entity_id: u32,
    _supertype: u32,
    _subtype: u32,
    i: &[u8],
) -> IResult<&[u8], PacketType> {
    let (i, packet) = parse_77(i)?;
    Ok((i, PacketType::Setup(packet)))
}

fn parse_unknown_entity_packet(
    entity_id: u32,
    supertype: u32,
    subtype: u32,
    payload: &[u8],
) -> IResult<&[u8], PacketType> {
    Ok((
        &[],
        PacketType::Entity(EntityPacket {
            supertype: supertype,
            entity_id: entity_id,
            subtype: subtype,
            payload: payload,
        }),
    ))
}

fn debug_packet(
    entity_id: u32,
    supertype: u32,
    subtype: u32,
    payload: &[u8],
) -> IResult<&[u8], PacketType> {
    if payload.len() > 0 {
        let (i, cnt) = be_u8(payload)?;
        if cnt as usize * 8 == i.len() {
            println!("Found needle?");
        }
    }

    println!(
        "Received {}-byte 0x{:x} 0x{:x} packet:",
        payload.len(),
        supertype,
        subtype
    );
    hexdump::hexdump(payload);
    Ok((
        &[],
        PacketType::Entity(EntityPacket {
            supertype: supertype,
            entity_id: entity_id,
            subtype: subtype,
            payload: payload,
        }),
    ))
}

fn lookup_entity_fn(
    version: u32,
    supertype: u32,
    subtype: u32,
) -> Option<fn(u32, u32, u32, &[u8]) -> IResult<&[u8], PacketType>> {
    let fn_0 = || parse_unknown_entity_packet;
    let fn_2571457 = || {
        // 0.9.4
        match (supertype, subtype) {
            (0x8, 0x76) => parse_chat_packet,
            (0x8, 0x77) => parse_setup_packet,
            (0x8, 0x3c) | (0x8, 0x3d) => parse_timing_packet,
            (0x8, 0x79) => parse_8_79_packet,
            (0x8, 0x63) => parse_artillery_hit_packet,
            (0x8, 0xc) => parse_banner_packet,
            (0x8, 0x35) => parse_damage_received_packet,
            _ => parse_unknown_entity_packet,
        }
    };
    let fn_2591463 = || {
        // 0.9.5.0
        match (supertype, subtype) {
            (0x8, 0x78) => parse_chat_packet,
            (0x8, 0x79) => parse_setup_packet,
            (0x8, 0x3e) | (0x8, 0x3f) => parse_timing_packet,
            (0x8, 0x7b) => parse_8_79_packet,
            (0x8, 0x64) => parse_artillery_hit_packet,
            (0x8, 0xc) => parse_banner_packet,
            (0x8, 0x35) => parse_damage_received_packet, // TODO: This needs better verification
            (0x8, 0x53) => parse_ship_destroyed_packet,
            (0x8, 0x58) => parse_voiceline_packet,
            _ => parse_unknown_entity_packet,
        }
    };
    let fn_2643263 = fn_2591463; // 0.9.5.1
    let fn_2666186 = fn_2643263; // 0.9.6.0
    let fn_2697511 = fn_2666186; // 0.9.6.1
    let fn_2744482 = fn_2697511; // 0.9.7.0
    let fn_2832630 = fn_2744482; // 0.9.8
    let fn_3747819 = || {
        // 0.10.3
        match (supertype, subtype) {
            (0x8, 0x78) => parse_chat_packet,
            /*(0x8, 0x79) => parse_setup_packet,
            (0x8, 0x3e) | (0x8, 0x3f) => parse_timing_packet,
            (0x8, 0x7b) => parse_8_79_packet,
            (0x8, 0x64) => parse_artillery_hit_packet,
            (0x8, 0xc) => parse_banner_packet,
            (0x8, 0x35) => parse_damage_received_packet, // TODO: This needs better verification
            (0x8, 0x53) => parse_ship_destroyed_packet,
            (0x8, 0x58) => parse_voiceline_packet,*/
            _ => parse_unknown_entity_packet,
        }
    };

    match version {
        0 => Some(fn_0()),
        2571457 => Some(fn_2571457()),
        2591463 => Some(fn_2591463()),
        2643263 => Some(fn_2643263()),
        2666186 => Some(fn_2666186()),
        2697511 => Some(fn_2697511()),
        2744482 => Some(fn_2744482()),
        2832630 => Some(fn_2832630()),
        3747819 => Some(fn_3747819()),
        _ => {
            //Err(error_from_kind(ErrorKind::UnsupportedReplayVersion(version)))
            None
            //panic!("Got unknown version number {}!", version);
        }
    }
}

fn parse_entity_packet(version: u32, supertype: u32, i: &[u8]) -> IResult<&[u8], PacketType> {
    let (i, entity_id) = le_u32(i)?;
    let (i, subtype) = le_u32(i)?;
    let (i, payload_length) = le_u32(i)?;
    let (i, payload) = take(payload_length)(i)?;
    let verfn = match lookup_entity_fn(version, supertype, subtype) {
        Some(x) => x,
        None => {
            return Err(failure_from_kind(ErrorKind::UnsupportedReplayVersion(
                version,
            )))
        }
    };
    let (remaining, packet) = verfn(entity_id, supertype, subtype, payload)?;
    if remaining.len() != 0 {
        warn!(
            "Parsing entity packet 0x{:x}.0x{:x} left {} bytes at end of stream",
            supertype,
            subtype,
            remaining.len()
        );
    }
    Ok((i, packet))
}

fn parse_position_packet(i: &[u8]) -> IResult<&[u8], PacketType> {
    let (i, pid) = le_u32(i)?;
    let (i, zero) = le_u32(i)?;
    if zero != 0 {
        panic!("What does this field mean?");
    }
    let (i, x) = le_f32(i)?;
    let (i, y) = le_f32(i)?;
    let (i, z) = le_f32(i)?;
    let (i, rot_x) = be_u32(i)?;
    let (i, rot_y) = be_u32(i)?;
    let (i, rot_z) = be_u32(i)?;
    let (i, a) = le_f32(i)?;
    let (i, b) = le_f32(i)?;
    let (i, c) = le_f32(i)?;
    let (i, extra) = be_u8(i)?;
    Ok((
        i,
        PacketType::Position(PositionPacket {
            pid,
            x,
            y,
            z,
            rot_x,
            rot_y,
            rot_z,
            a,
            b,
            c,
            extra,
        }),
    ))
}

fn parse_unknown_packet(i: &[u8], payload_size: u32) -> IResult<&[u8], PacketType> {
    let (i, contents) = take(payload_size)(i)?;
    Ok((i, PacketType::Unknown(contents)))
}

fn parse_naked_packet(version: u32, packet_type: u32, i: &[u8]) -> IResult<&[u8], PacketType> {
    let (i, payload) = match packet_type {
        0x7 | 0x8 => parse_entity_packet(version, packet_type, i)?,
        0xA => parse_position_packet(i)?,
        /*0x24 => {
            parse_type_24_packet(i)?
        }*/
        0x2b => parse_player_orientation_packet(i)?,
        _ => parse_unknown_packet(i, i.len().try_into().unwrap())?,
    };
    Ok((i, payload))
}

fn parse_packet(version: u32, i: &[u8]) -> IResult<&[u8], Packet> {
    let (i, packet_size) = le_u32(i)?;
    let (i, packet_type) = le_u32(i)?;
    let (i, clock) = le_f32(i)?;
    let (remaining, i) = take(packet_size)(i)?;
    let raw = i;
    /*let (i, payload) = match packet_type {
            0x7 | 0x8 => parse_entity_packet(version, packet_type, i)?,
            0xA => parse_position_packet(i)?,
            /*0x24 => {
                parse_type_24_packet(i)?
            }*/
            0x2b => parse_player_orientation_packet(i)?,
            _ => parse_unknown_packet(i, packet_size)?,
    };*/
    let (i, payload) = match parse_naked_packet(version, packet_type, i) {
        Ok(x) => x,
        Err(nom::Err::Failure(Error {
            kind: ErrorKind::UnsupportedReplayVersion(n),
            ..
        })) => {
            return Err(failure_from_kind(ErrorKind::UnsupportedReplayVersion(n)));
        }
        Err(e) => {
            (
                &i[0..0], // Empty reference
                PacketType::Invalid(InvalidPacket {
                    message: format!("{:?}", e),
                    raw: i,
                }),
            )
        }
    };
    assert!(i.len() == 0);
    Ok((
        remaining,
        Packet {
            packet_size: packet_size,
            packet_type: packet_type,
            clock: clock,
            payload: payload,
            raw: raw,
        },
    ))
}

pub fn parse_packets(version: u32, i: &[u8]) -> Result<Vec<Packet>, ErrorKind> {
    let mut i = i;
    let mut v = vec![];
    while i.len() > 0 {
        let (remaining, packet) = parse_packet(version, i)?;
        i = remaining;
        v.push(packet);
    }
    Ok(v)
}
