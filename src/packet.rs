use nom::{bytes::complete::take, bytes::complete::tag, named, do_parse, take, tag, number::complete::be_u16, number::complete::le_u16, number::complete::be_u8, alt, cond, number::complete::be_u24, char, opt, one_of, take_while, length_data, many1, complete, number::complete::le_u32, number::complete::le_f32, multi::many0, number::complete::be_u32, multi::count};
use std::collections::HashMap;
use std::convert::TryInto;
use plotters::prelude::*;
use image::{imageops::FilterType, ImageFormat};

//mod error;
//mod wowsreplay;

use crate::error::*;
//use crate::wowsreplay::*;

#[derive(Debug)]
pub struct PositionPacket {
    pub pid: u32,
    //clock: f32,
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
    //raw: &'a [u8],
}

#[derive(Debug)]
pub struct EntityPacket<'a> {
    pub supertype: u32,
    pub entity_id: u32,
    pub subtype: u32,
    pub payload: &'a [u8],
}

#[derive(Debug)]
pub struct ChatPacket<'a> {
    pub entity_id: u32, // TODO: Is entity ID different than sender ID?
    pub sender_id: u32,
    pub audience: &'a str,
    pub message: &'a str,
}

#[derive(Debug)]
pub struct TimingPacket {
    pub time: u32,
}

#[derive(Debug)]
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

/// This appears to be camera position.
/// PID (or maybe sub_object_id?) appears to be what the camera is attached to.
/// Then there is a follow-up packet (of this type) which describes the
/// (relative?) position
#[derive(Debug)]
pub struct Type2bPacket {
    pub pid: u32,
    pub sub_object_id: u32,
    pub f0: f32, // Appears to be a coordinate - X or Z
    pub f1: f32, // Unknown? (possibly Y?)
    pub f2: f32, // Appears to be a coordinate - X or Z
    pub f3: f32, // Appears to be a rotation of some sort? (wraps at pi)
    pub f4: f32,
    pub f5: f32,
}

#[derive(Debug)]
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

#[derive(Debug)]
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
}

#[derive(Debug)]
pub struct DamageReceivedPacket {
    recipient: u32,
    damage: Vec<(u32, f32)>,
}

#[derive(Debug)]
pub enum PacketType<'a> {
    Position(PositionPacket),
    Entity(EntityPacket<'a>), // 0x7 and 0x8 are known to be of this type
    Chat(ChatPacket<'a>),
    Timing(TimingPacket),
    ArtilleryHit(ArtilleryHitPacket<'a>),
    Banner(Banner),
    DamageReceived(DamageReceivedPacket),
    Type24(Type24Packet),
    Type2b(Type2bPacket),
    Type8_79(Vec<(u32, u32)>),
    Unknown(&'a [u8]),
}

#[derive(Debug)]
pub struct Packet<'a> {
    pub packet_size: u32,
    pub packet_type: u32,
    pub clock: f32,
    pub payload: PacketType<'a>,
    pub raw: &'a [u8],
}

fn parse_type_2b_packet(i: &[u8]) -> IResult<&[u8], PacketType> {
    assert!(i.len() == 0x20);
    let (i, pid) = le_u32(i)?;
    let (i, sub_object_id) = le_u32(i)?;
    let (i, f0) = le_f32(i)?;
    let (i, f1) = le_f32(i)?;
    let (i, f2) = le_f32(i)?;
    let (i, f3) = le_f32(i)?;
    let (i, f4) = le_f32(i)?;
    let (i, f5) = le_f32(i)?;
    Ok((
        i,
        PacketType::Type2b(Type2bPacket{
            pid, sub_object_id, f0, f1, f2, f3, f4, f5,
        })
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
        PacketType::Type24(Type24Packet{
            f0, f1, f2, f3, f4, f5, f6, f7, f8, f9, f10, f11, f12, f13,
        })
    ))
}

/// Note! There are actually two types of timing packets, which always seem to
/// mirror each other. ID N is immediately followed by ID N+1 w/ matching
/// counter. The counter seems to increment ~ once per ms
fn parse_timing_packet(entity_id: u32, supertype: u32, subtype: u32, i: &[u8]) -> IResult<&[u8], PacketType> {
    let (i, time) = le_u32(i)?;
    let (i, zero) = le_u32(i)?;
    // TODO: Re-enable
    //assert!(zero == 0); // What does this field mean?
    Ok((
        i,
        PacketType::Timing(TimingPacket{
            time,
        })
    ))
}

fn parse_chat_packet(entity_id: u32, supertype: u32, subtype: u32, i: &[u8]) -> IResult<&[u8], PacketType> {
    let (i, sender) = le_u32(i)?;
    let (i, audience_len) = be_u8(i)?;
    let (i, audience) = take(audience_len)(i)?;
    let (i, message_len) = be_u8(i)?;
    let (i, message) = take(message_len)(i)?;
    Ok((
        i,
        PacketType::Chat(ChatPacket{
            entity_id: entity_id,
            sender_id: sender,
            audience: std::str::from_utf8(audience).unwrap(),
            message: std::str::from_utf8(message).unwrap(),
        })
    ))
}

fn parse_8_79_subobject(i: &[u8]) -> IResult<&[u8], (u32, u32)> {
    let (i, pid) = le_u32(i)?;
    let (i, data) = le_u32(i)?;
    Ok((
        i,
        (pid, data),
    ))
}

fn parse_8_79_packet(entity_id: u32, supertype: u32, subtype: u32, i: &[u8]) -> IResult<&[u8], PacketType> {
    //hexdump::hexdump(i);
    let (i, num_objects) = be_u8(i)?;
    //println!("Found {} objects", num_objects);
    let (i, objects) = count(parse_8_79_subobject, num_objects.try_into().unwrap())(i)?;
    let (i, b) = be_u8(i)?;
    assert!(b == 0); // What does this field do?
    assert!(i.len() == 0);
    Ok((
        i,
        PacketType::Type8_79(objects),
    ))
}

fn parse_artillery_hit_packet(entity_id: u32, supertype: u32, subtype: u32, i: &[u8]) -> IResult<&[u8], PacketType> {
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
        0 => { false },
        0xff => { true },
        _ => {
            hexdump::hexdump(raw);
            panic!(format!("Got unknown value 0x{:x} for secondary bitfield!", bitmask5));
        }
    };
    let is_he = false;//(bitmask0 & (1 << 22)) != 0;
    let is_incoming = (bitmask1 & (1 << 0)) != 0;
    assert!(i.len() == 0);
    Ok((
        i,
        PacketType::ArtilleryHit(ArtilleryHitPacket{
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
        })
    ))
}

fn parse_banner_packet(entity_id: u32, supertype: u32, subtype: u32, i: &[u8]) -> IResult<&[u8], PacketType> {
    let (i, banner) = be_u8(i)?;
    let banner = match banner {
        3 => Banner::PlaneShotDown,
        4 => Banner::Incapacitation,
        6 => Banner::SetFire,
        8 => Banner::Citadel,
        13 => Banner::SecondaryHit,
        14 => Banner::OverPenetration,
        15 => Banner::Penetration,
        16 => Banner::NonPenetration,
        17 => Banner::Ricochet,
        28 => Banner::TorpedoProtectionHit,
        _ => {
            // TODO: Put the panic back
            //panic!("Got unknown banner type {}!", banner);
            Banner::Ricochet
        }
    };
    Ok((
        i,
        PacketType::Banner(banner),
    ))
}

fn parse_damage_received_part(i: &[u8]) -> IResult<&[u8], (u32, f32)> {
    let (i, pid) = le_u32(i)?;
    let (i, damage) = le_f32(i)?;
    Ok((i, (pid, damage)))
}

fn parse_damage_received_packet(entity_id: u32, supertype: u32, subtype: u32, i: &[u8]) -> IResult<&[u8], PacketType> {
    let (i, cnt) = be_u8(i)?;
    if cnt == 0 {
        // TODO: It's not clear what's actually happening here
        // This behaviour is ostensible new to 0.9.5(.1?)
        //hexdump::hexdump(i);
        assert!(i.len() == 5);
        return Ok((
            &[],
            PacketType::Entity(EntityPacket{
                supertype, entity_id, subtype, payload: i,
            })
        ))
    }
    if i.len() != 8 * cnt as usize {
        println!("Unclear damage recv'd packet: cnt={}", cnt);
        hexdump::hexdump(i);
        //panic!();
        return Ok((
            &[],
            PacketType::Entity(EntityPacket{
                supertype, entity_id, subtype, payload: i,
            })
        ))
    }
    let (i, data) = count(parse_damage_received_part, cnt.try_into().unwrap())(i)?;
    assert!(i.len() == 0);
    Ok((
        i,
        PacketType::DamageReceived(DamageReceivedPacket{
            recipient: entity_id,
            damage: data,
        })
    ))
}

fn parse_unknown_entity_packet(entity_id: u32, supertype: u32, subtype: u32, payload: &[u8]) -> IResult<&[u8], PacketType> {
    Ok((
        &[],
        PacketType::Entity(EntityPacket{
            supertype: supertype,
            entity_id: entity_id,
            subtype: subtype,
            payload: payload,
        })
    ))
}

fn debug_packet(entity_id: u32, supertype: u32, subtype: u32, payload: &[u8]) -> IResult<&[u8], PacketType> {
    if payload.len() > 0 {
        let (i, cnt) = be_u8(payload)?;
        if cnt as usize * 8 == i.len() {
            println!("Found needle?");
        }
    }

    println!("Received {}-byte 0x{:x} 0x{:x} packet:", payload.len(), supertype, subtype);
    hexdump::hexdump(payload);
    Ok((
        &[],
        PacketType::Entity(EntityPacket{
            supertype: supertype,
            entity_id: entity_id,
            subtype: subtype,
            payload: payload,
        })
    ))
}

fn lookup_entity_fn(version: u32, supertype: u32, subtype: u32) -> fn(u32,u32,u32,&[u8]) -> IResult<&[u8], PacketType> {
    // Note: These subtype numbers seem to change with version (0x76 -> 0x79? 0x3d/0x3e -> 0x3e/0x3f?)
    let fn_2571457 = || { // 0.9.4
        match (supertype, subtype) {
            (0x8, 0x76) => parse_chat_packet,
            (0x8, 0x3c) | (0x8, 0x3d) => parse_timing_packet,
            (0x8, 0x79) => parse_8_79_packet,
            (0x8, 0x63) => parse_artillery_hit_packet,
            (0x8, 0xc) => parse_banner_packet,
            (0x8, 0x35) => parse_damage_received_packet,
            _ => parse_unknown_entity_packet,
        }
    };
    let fn_2643263 = || { // 0.9.5.1
        match (supertype, subtype) {
            (0x8, 0x78) => parse_chat_packet,
            (0x8, 0x3e) | (0x8, 0x3f) => parse_timing_packet,
            (0x8, 0x7b) => parse_8_79_packet,
            (0x8, 0x64) => parse_artillery_hit_packet,
            (0x8, 0xc) => parse_banner_packet,
            (0x8, 0x35) => parse_damage_received_packet, // TODO: This needs better verification
            _ => parse_unknown_entity_packet,
        }
    };

    match version {
        2571457 => {
            fn_2571457()
        }
        2643263 => {
            fn_2643263()
        }
        _ => {
            panic!("Got unknown version number {}!", version);
        }
    }
}

fn parse_entity_packet(version: u32, supertype: u32, i: &[u8]) -> IResult<&[u8], PacketType> {
    let (i, entity_id) = le_u32(i)?;
    let (i, subtype) = le_u32(i)?;
    let (i, payload_length) = le_u32(i)?;
    let (i, payload) = take(payload_length)(i)?;
    //println!("Parsing {}-byte 0x{:x} 0x{:x} packet", payload_length, supertype, subtype);
    let (remaining, packet) = lookup_entity_fn(version, supertype, subtype)(entity_id, supertype, subtype, payload)?;
    // TODO: Re-enable this assert
    //assert!(remaining.len() == 0);
    Ok((
        i,
        packet
    ))
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
        PacketType::Position(PositionPacket{
            pid, x, y, z, rot_x, rot_y, rot_z, a, b, c, extra,
        })
    ))
}

fn parse_unknown_packet(i: &[u8], payload_size: u32) -> IResult<&[u8], PacketType> {
    let (i, contents) = take(payload_size)(i)?;
    Ok((
        i,
        PacketType::Unknown(contents)
    ))
}

fn parse_packet(version: u32, i: &[u8]) -> IResult<&[u8], Packet> {
    let (i, packet_size) = le_u32(i)?;
    let (i, packet_type) = le_u32(i)?;
    let (i, clock) = le_f32(i)?;
    let (remaining, i) = take(packet_size)(i)?;
    let raw = i;
    let (i, payload) = match packet_type {
        0x7 | 0x8 => {
            parse_entity_packet(version, packet_type, i)?
        }
        0xA => {
            parse_position_packet(i)?
        }
        /*0x24 => {
            parse_type_24_packet(i)?
        }*/
        0x2b => {
            parse_type_2b_packet(i)?
        }
        _ => {
            parse_unknown_packet(i, packet_size)?
        }
    };
    assert!(i.len() == 0);
    Ok((
        remaining,
        Packet{
            packet_size: packet_size,
            packet_type: packet_type,
            clock: clock,
            payload: payload,
            raw: raw,
        }
    ))
}

pub fn parse_packets(version: u32, i: &[u8]) -> IResult<&[u8], Vec<Packet>> {
    //many0(parse_packet)(i)
    let mut i = i;
    let mut v = vec!();
    while i.len() > 0 {
        //println!("Parsing remaining {} bytes...", i.len());
        let (remaining, packet) = parse_packet(version, i)?;
        //assert!(remaining.len() == 0);
        i = remaining;
        v.push(packet);
    }
    Ok((i, v))
}
