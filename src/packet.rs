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
pub enum PacketType<'a> {
    Position(PositionPacket),
    Entity(EntityPacket<'a>), // 0x7 and 0x8 are known to be of this type
    Chat(ChatPacket<'a>),
    Timing(TimingPacket),
    ArtilleryHit(ArtilleryHitPacket<'a>),
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
fn parse_timing_packet(i: &[u8]) -> IResult<&[u8], PacketType> {
    let (i, time) = le_u32(i)?;
    let (i, zero) = le_u32(i)?;
    assert!(zero == 0); // What does this field mean?
    Ok((
        i,
        PacketType::Timing(TimingPacket{
            time,
        })
    ))
}

fn parse_chat_packet(entity_id: u32, i: &[u8]) -> IResult<&[u8], PacketType> {
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

fn parse_8_79_packet(i: &[u8]) -> IResult<&[u8], PacketType> {
    //hexdump::hexdump(i);
    let (i, num_objects) = be_u8(i)?;
    let (i, objects) = count(parse_8_79_subobject, num_objects.try_into().unwrap())(i)?;
    let (i, b) = be_u8(i)?;
    assert!(b == 0); // What does this field do?
    assert!(i.len() == 0);
    Ok((
        i,
        PacketType::Type8_79(objects),
    ))
}

fn parse_artillery_hit_packet(i: &[u8]) -> IResult<&[u8], PacketType> {
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

fn parse_entity_packet(supertype: u32, i: &[u8]) -> IResult<&[u8], PacketType> {
    let (i, entity_id) = le_u32(i)?;
    //println!("Unknown: 0x{:x}", unknown);
    let (i, subtype) = le_u32(i)?; // Probably?
    let (i, payload_length) = le_u32(i)?;
    let (i, payload) = take(payload_length)(i)?;
    // Note: These subtype numbers seem to change with version (0x76 -> 0x79? 0x3d/0x3e -> 0x3e/0x3f?)
    if supertype == 0x8 {
        if subtype == 0x76 {
            Ok((
                i,
                parse_chat_packet(entity_id, payload)?.1
            ))
        } else if subtype == 0x3d || subtype == 0x3e {
            Ok((
                i,
                parse_timing_packet(payload)?.1
            ))
        /*} else if subtype == 0x79 {
            Ok((
                i,
                parse_8_79_packet(payload)?.1
        ))*/
        } else if subtype == 0x63 {
            Ok((
                i,
                parse_artillery_hit_packet(payload)?.1
            ))
        } else {
            Ok((
                i,
                PacketType::Entity(EntityPacket{
                    supertype: supertype,
                    entity_id: entity_id,
                    subtype: subtype,
                    payload: payload,
                })
            ))
        }
    } else {
        Ok((
            i,
            PacketType::Entity(EntityPacket{
                supertype: supertype,
                entity_id: entity_id,
                subtype: subtype,
                payload: payload,
            })
        ))
    }
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

fn parse_packet(i: &[u8]) -> IResult<&[u8], Packet> {
    let (i, packet_size) = le_u32(i)?;
    let (i, packet_type) = le_u32(i)?;
    let (i, clock) = le_f32(i)?;
    let (remaining, i) = take(packet_size)(i)?;
    let raw = i;
    let (i, payload) = match packet_type {
        0x7 | 0x8 => {
            parse_entity_packet(packet_type, i)?
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

pub fn parse_packets(i: &[u8]) -> IResult<&[u8], Vec<Packet>> {
    many0(parse_packet)(i)
}
