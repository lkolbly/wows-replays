use kinded::Kinded;
use nom::{
    bytes::complete::take, number::complete::le_f32, number::complete::le_i32,
    number::complete::le_i64, number::complete::le_u16, number::complete::le_u32,
    number::complete::le_u8,
};

use serde::Serialize;
use std::collections::HashMap;
use std::convert::TryInto;

use crate::error::*;
use crate::rpc::entitydefs::*;
use crate::rpc::typedefs::ArgValue;

#[derive(Debug, Serialize, Clone)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn parse(i: &[u8]) -> IResult<&[u8], Self> {
        let (i, x) = le_f32(i)?;
        let (i, y) = le_f32(i)?;
        let (i, z) = le_f32(i)?;
        Ok((i, Vec3 { x, y, z }))
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct Rot3 {
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
}

impl Rot3 {
    pub fn parse(i: &[u8]) -> IResult<&[u8], Self> {
        let (i, roll) = le_f32(i)?;
        let (i, pitch) = le_f32(i)?;
        let (i, yaw) = le_f32(i)?;
        Ok((i, Rot3 { roll, pitch, yaw }))
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct PositionPacket {
    pub pid: u32,
    pub position: Vec3,
    pub position_error: Vec3,
    pub rotation: Rot3,
    pub is_error: bool,
}

#[derive(Debug, Serialize)]
pub struct EntityPacket<'replay> {
    pub supertype: u32,
    pub entity_id: u32,
    pub subtype: u32,
    pub payload: &'replay [u8],
}

#[derive(Debug, Serialize)]
pub struct EntityPropertyPacket<'argtype> {
    pub entity_id: u32,
    pub property: &'argtype str,
    pub value: ArgValue<'argtype>,
}

#[derive(Debug, Serialize)]
pub struct EntityMethodPacket<'argtype> {
    pub entity_id: u32,
    pub method: &'argtype str,
    pub args: Vec<ArgValue<'argtype>>,
}

#[derive(Debug, Serialize)]
pub struct EntityCreatePacket<'argtype> {
    pub entity_id: u32,
    pub entity_type: &'argtype str,
    pub space_id: u32,
    pub vehicle_id: u32,
    pub position: Vec3,
    pub rotation: Rot3,
    pub state_length: u32,
    pub props: HashMap<&'argtype str, crate::rpc::typedefs::ArgValue<'argtype>>,
}

/// Note that this packet frequently appears twice - it appears that it
/// describes both the player's boat location/orientation as well as the
/// camera orientation. When the camera is attached to an object, the ID of
/// that object will be given in the parent_id field.
#[derive(Debug, Serialize, Clone)]
pub struct PlayerOrientationPacket {
    pub pid: u32,
    pub parent_id: u32,
    pub position: Vec3,
    pub rotation: Rot3,
}

#[derive(Debug, Serialize)]
pub struct InvalidPacket<'a> {
    message: String,
    raw: &'a [u8],
}

#[derive(Debug, Serialize)]
pub struct BasePlayerCreatePacket<'replay, 'argtype> {
    pub entity_id: u32,
    pub entity_type: &'argtype str,
    pub state: &'replay [u8],
}

#[derive(Debug, Serialize)]
pub struct CellPlayerCreatePacket<'replay> {
    pub entity_id: u32,
    pub space_id: u32,
    pub unknown: u16,
    pub vehicle_id: u32,
    pub position: Vec3,
    pub rotation: Rot3,
    pub value: &'replay [u8],
}

#[derive(Debug, Serialize)]
pub struct EntityLeavePacket {
    pub entity_id: u32,
}

#[derive(Debug, Serialize)]
pub struct EntityEnterPacket {
    pub entity_id: u32,
    pub space_id: u32,
    pub vehicle_id: u32,
}

#[derive(Debug, Serialize)]
pub struct PropertyUpdatePacket<'argtype> {
    /// Indicates the entity to update the property on
    pub entity_id: i32,
    /// Indicates the property to update. Note that some properties have many
    /// sub-properties.
    pub property: &'argtype str,
    /// Indicates the update command to perform.
    pub update_cmd: crate::nested_property_path::PropertyNesting<'argtype>,
}

#[derive(Debug, Serialize)]
pub struct CameraPacket {
    pub unknown: Vec3,
    pub unknown2: u32,
    pub absolute_position: Vec3,
    pub fov: f32,
    pub position: Vec3,
    pub rotation: Rot3,
}

#[derive(Debug, Serialize)]
pub struct CruiseState {
    pub key: u32,
    pub value: i32,
}

#[derive(Debug, Serialize)]
pub struct MapPacket<'replay> {
    pub space_id: u32,
    pub arena_id: i64,
    pub unknown1: u32,
    pub unknown2: u32,
    pub blob: &'replay [u8],
    pub map_name: &'replay str,
    /// Note: We suspect that this matrix is always the unit matrix, hence
    /// we don't spend the computation to parse it.
    pub matrix: &'replay [u8],
    pub unknown: u8, // bool?
}

#[derive(Debug, Serialize, Kinded)]
pub enum PacketType<'replay, 'argtype> {
    Position(PositionPacket),
    BasePlayerCreate(BasePlayerCreatePacket<'replay, 'argtype>),
    CellPlayerCreate(CellPlayerCreatePacket<'replay>),
    EntityEnter(EntityEnterPacket),
    EntityLeave(EntityLeavePacket),
    EntityCreate(EntityCreatePacket<'argtype>),
    EntityProperty(EntityPropertyPacket<'argtype>),
    EntityMethod(EntityMethodPacket<'argtype>),
    PropertyUpdate(PropertyUpdatePacket<'argtype>),
    PlayerOrientation(PlayerOrientationPacket),
    CruiseState(CruiseState),
    Version(String),
    Camera(CameraPacket),
    CameraMode(u32),
    CameraFreeLook(u8),
    Map(MapPacket<'replay>),
    BattleResults(&'replay str),
    Unknown(&'replay [u8]),

    /// These are packets which we thought we understood, but couldn't parse
    Invalid(InvalidPacket<'replay>),
}

#[derive(Debug, Serialize)]
pub struct Packet<'replay, 'argtype> {
    pub packet_size: u32,
    pub packet_type: u32,
    pub clock: f32,
    pub payload: PacketType<'replay, 'argtype>,
    pub raw: &'replay [u8],
}

#[derive(Debug)]
struct Entity<'argtype> {
    entity_type: u16,
    properties: Vec<ArgValue<'argtype>>,
}

pub struct Parser<'argtype> {
    specs: &'argtype Vec<EntitySpec>,
    entities: HashMap<u32, Entity<'argtype>>,
}

impl<'argtype> Parser<'argtype> {
    pub fn new(entities: &'argtype Vec<EntitySpec>) -> Parser {
        Parser {
            specs: entities,
            entities: HashMap::new(),
        }
    }

    fn parse_entity_property_packet<'a, 'b>(
        &'b self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'argtype>> {
        let (i, entity_id) = le_u32(i)?;
        let (i, prop_id) = le_u32(i)?;
        let (i, payload_length) = le_u32(i)?;
        let (i, payload) = take(payload_length)(i)?;

        let entity_type = self.entities.get(&entity_id).unwrap().entity_type;
        let spec = &self.specs[entity_type as usize - 1].properties[prop_id as usize];

        let (_, pval) = spec.prop_type.parse_value(payload).unwrap();

        Ok((
            i,
            PacketType::EntityProperty(EntityPropertyPacket {
                entity_id,
                property: &spec.name,
                value: pval,
            }),
        ))
    }

    fn parse_entity_method_packet<'a, 'b>(
        &'b self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        let (i, entity_id) = le_u32(i)?;
        let (i, method_id) = le_u32(i)?;
        let (i, payload_length) = le_u32(i)?;
        let (i, payload) = take(payload_length)(i)?;
        assert!(i.len() == 0);

        let entity_type = self.entities.get(&entity_id).unwrap().entity_type;

        let spec = &self.specs[entity_type as usize - 1].client_methods[method_id as usize];

        let mut i = payload;
        let mut args = vec![];
        for (idx, arg) in spec.args.iter().enumerate() {
            let (new_i, pval) = match arg.parse_value(i) {
                Ok(x) => x,
                Err(e) => {
                    return Err(failure_from_kind(crate::ErrorKind::UnableToParseRpcValue {
                        method: format!("{}", spec.name),
                        argnum: idx,
                        argtype: format!("{:?}", arg),
                        packet: i.to_vec(),
                        error: format!("{:?}", e),
                    }));
                }
            };
            args.push(pval);
            i = new_i;
        }

        Ok((
            i,
            PacketType::EntityMethod(EntityMethodPacket {
                entity_id,
                method: &spec.name,
                args,
            }),
        ))
    }

    fn parse_battle_results<'replay, 'b>(
        &'b mut self,
        i: &'replay [u8],
    ) -> IResult<&'replay [u8], PacketType<'replay, 'argtype>> {
        let (i, len) = le_u32(i)?;
        assert_eq!(len as usize, i.len());
        let (i, battle_results) = take(len)(i)?;

        let results = std::str::from_utf8(battle_results).map_err(|_| {
            failure_from_kind(crate::ErrorKind::ParsingFailure(
                "Invalid UTF-8 data in battle results".to_string(),
            ))
        })?;

        Ok((i, PacketType::BattleResults(results)))
    }

    fn parse_nested_property_update<'replay, 'b>(
        &'b mut self,
        i: &'replay [u8],
    ) -> IResult<&'replay [u8], PacketType<'replay, 'argtype>> {
        let (i, entity_id) = le_u32(i)?;
        let (i, is_slice) = le_u8(i)?;
        let (i, payload_size) = le_u8(i)?;
        let (i, unknown) = take(3usize)(i)?;
        assert_eq!(unknown, [0, 0, 0]); // Note: This is almost certainly the upper 3 bytes of a u32
        let payload = i;
        assert_eq!(payload_size as usize, payload.len());

        let entity = self.entities.get_mut(&entity_id).unwrap();
        let entity_type = entity.entity_type;

        let spec = &self.specs[entity_type as usize - 1];

        assert!(is_slice & 0xFE == 0);

        let mut reader = bitreader::BitReader::new(payload);
        let cont = reader.read_u8(1).unwrap();
        assert!(cont == 1);
        let prop_idx = reader
            .read_u8(spec.properties.len().next_power_of_two().trailing_zeros() as u8)
            .unwrap();
        if prop_idx as usize >= entity.properties.len() {
            // This is almost certainly a nested property set on the player avatar.
            // Currently, we assume that all properties are created when the entity is
            // created. However, apparently the properties can go un-initialized at the
            // beginning, and then later get created by a nested property update.
            //
            // We should do two things:
            // - Store the entity's properties as a HashMap
            // - Separate finding the path from updating the property value, and then here
            //   we can create the entry if the property hasn't been created yet.
            return Err(failure_from_kind(
                crate::ErrorKind::UnsupportedInternalPropSet {
                    entity_id,
                    entity_type: spec.name.clone(),
                    payload: payload.to_vec(),
                },
            ));
        }

        let update_cmd = crate::nested_property_path::get_nested_prop_path_helper(
            is_slice & 0x1 == 1,
            &spec.properties[prop_idx as usize].prop_type,
            &mut entity.properties[prop_idx as usize],
            reader,
        );

        Ok((
            i,
            PacketType::PropertyUpdate(PropertyUpdatePacket {
                entity_id: entity_id as i32,
                update_cmd,
                property: &spec.properties[prop_idx as usize].name,
            }),
        ))
    }

    fn parse_version_packet<'replay, 'b>(
        &'b self,
        i: &'replay [u8],
    ) -> IResult<&'replay [u8], PacketType<'replay, 'argtype>> {
        let (i, len) = le_u32(i)?;
        let (i, data) = take(len)(i)?;
        Ok((
            i,
            PacketType::Version(std::str::from_utf8(data).unwrap().to_string()),
        ))
    }

    fn parse_camera_mode_packet<'replay, 'b>(
        &'b self,
        i: &'replay [u8],
    ) -> IResult<&'replay [u8], PacketType<'replay, 'argtype>> {
        let (i, mode) = le_u32(i)?;
        Ok((i, PacketType::CameraMode(mode)))
    }

    fn parse_camera_freelook_packet<'replay, 'b>(
        &'b self,
        i: &'replay [u8],
    ) -> IResult<&'replay [u8], PacketType<'replay, 'argtype>> {
        let (i, freelook) = le_u8(i)?;
        Ok((i, PacketType::CameraFreeLook(freelook)))
    }

    fn parse_position_packet<'a, 'b>(
        &'b self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        let (i, pid) = le_u32(i)?;
        let (i, zero) = le_u32(i)?;
        if zero != 0 {
            panic!("What does this field mean?");
        }
        let (i, position) = Vec3::parse(i)?;
        let (i, position_error) = Vec3::parse(i)?;
        let (i, rotation) = Rot3::parse(i)?;
        let (i, is_error_byte) = le_u8(i)?;
        let is_error = is_error_byte != 0;
        Ok((
            i,
            PacketType::Position(PositionPacket {
                pid,
                position,
                position_error,
                rotation,
                is_error,
            }),
        ))
    }

    fn parse_player_orientation_packet<'a, 'b>(
        &'b self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        assert!(i.len() == 0x20);
        let (i, pid) = le_u32(i)?;
        let (i, parent_id) = le_u32(i)?;
        let (i, position) = Vec3::parse(i)?;
        let (i, rotation) = Rot3::parse(i)?;
        Ok((
            i,
            PacketType::PlayerOrientation(PlayerOrientationPacket {
                pid,
                parent_id,
                position,
                rotation,
            }),
        ))
    }

    fn parse_camera_packet<'a, 'b>(&'b self, i: &'a [u8]) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        let (i, unknown) = Vec3::parse(i)?;
        let (i, unknown2) = le_u32(i)?;
        let (i, absolute_position) = Vec3::parse(i)?;
        let (i, fov) = le_f32(i)?;
        let (i, position) = Vec3::parse(i)?;
        let (i, rotation) = Rot3::parse(i)?;
        Ok((
            i,
            PacketType::Camera(CameraPacket {
                unknown,
                unknown2,
                absolute_position,
                fov,
                position,
                rotation,
            }),
        ))
    }

    fn parse_unknown_packet<'a, 'b>(
        &'b self,
        i: &'a [u8],
        payload_size: u32,
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        let (i, contents) = take(payload_size)(i)?;
        Ok((i, PacketType::Unknown(contents)))
    }

    fn parse_base_player_create<'a, 'b>(
        &'b mut self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        let (i, entity_id) = le_u32(i)?;
        let (i, entity_type) = le_u16(i)?;
        let (i, state) = take(i.len())(i)?;
        let spec = &self.specs[entity_type as usize - 1];
        self.entities.insert(
            entity_id,
            Entity {
                entity_type,
                // TODO: Parse the state
                properties: vec![],
            },
        );
        Ok((
            i,
            PacketType::BasePlayerCreate(BasePlayerCreatePacket {
                entity_id,
                entity_type: &spec.name,
                state,
            }),
        ))
    }

    fn parse_entity_create<'a, 'b>(
        &'b mut self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        let (i, entity_id) = le_u32(i)?;
        let (i, entity_type) = le_u16(i)?;
        let (i, vehicle_id) = le_u32(i)?;
        let (i, space_id) = le_u32(i)?;
        let (i, position) = Vec3::parse(i)?;
        let (i, rotation) = Rot3::parse(i)?;
        let (i, state_length) = le_u32(i)?;
        let (_, state) = take(i.len())(i)?;
        if self.entities.contains_key(&entity_id) {
            //println!("DBG: Entity {} got created twice!", entity_id);
        }

        let (i, num_props) = le_u8(state)?;
        let mut i = i;
        let mut props: HashMap<&str, _> = HashMap::new();
        let mut stored_props: Vec<_> = vec![];
        for _ in 0..num_props {
            let (new_i, prop_id) = le_u8(i)?;
            let spec = &self.specs[entity_type as usize - 1].properties[prop_id as usize];
            let (new_i, value) = match spec.prop_type.parse_value(new_i) {
                Ok(x) => x,
                Err(e) => {
                    return Err(failure_from_kind(crate::ErrorKind::UnableToParseRpcValue {
                        method: format!("EntityCreate::{}", spec.name),
                        argnum: prop_id as usize,
                        argtype: format!("{:?}", spec),
                        packet: i.to_vec(),
                        error: format!("{:?}", e),
                    }));
                }
            };
            i = new_i;
            stored_props.push(value.clone());
            props.insert(&spec.name, value);
        }

        self.entities.insert(
            entity_id,
            Entity {
                entity_type,
                properties: stored_props,
            },
        );

        Ok((
            i,
            PacketType::EntityCreate(EntityCreatePacket {
                entity_id,
                entity_type: &self.specs[entity_type as usize - 1].name,
                space_id,
                vehicle_id,
                position,
                rotation,
                state_length,
                props,
            }),
        ))
    }

    fn parse_cell_player_create<'a, 'b>(
        &'b mut self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        let (i, entity_id) = le_u32(i)?;
        let (i, space_id) = le_u32(i)?;
        // let (i, _unknown) = le_u16(i)?;
        let (i, vehicle_id) = le_u32(i)?;
        let (i, position) = Vec3::parse(i)?;
        let (i, rotation) = Rot3::parse(i)?;
        let (i, vlen) = le_u32(i)?;
        let (i, value) = take(vlen)(i)?;

        if !self.entities.contains_key(&entity_id) {
            panic!(
                "Cell player, entity id {}, was created before base player!",
                entity_id
            );
        }

        // The value can be parsed into all internal properties
        /*println!(
            "{} {} {} {} {},{},{} {},{},{} value.len()={}",
            entity_id,
            space_id,
            5, //unknown,
            vehicle_id,
            posx,
            posy,
            posz,
            dirx,
            diry,
            dirz,
            value.len()
        );*/
        let entity_type = self.entities.get(&entity_id).unwrap().entity_type;
        let spec = &self.specs[entity_type as usize - 1];
        let mut value = value;
        let mut prop_values = vec![];
        for property in spec.internal_properties.iter() {
            //println!("{}: {}", idx, property.name);
            //println!("{:#?}", property.prop_type);
            //println!("{:?}", value);
            let (new_value, prop_value) = property.prop_type.parse_value(value).unwrap();
            //println!("{:?}", prop_value);
            value = new_value;
            prop_values.push(prop_value);
        }
        //println!("CellPlayerCreate properties: {:?}", prop_values);

        Ok((
            i,
            PacketType::CellPlayerCreate(CellPlayerCreatePacket {
                entity_id,
                vehicle_id,
                space_id,
                position,
                rotation,
                unknown: 5,
                value,
            }),
        ))
    }

    fn parse_entity_leave<'a, 'b>(&'b self, i: &'a [u8]) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        let (i, entity_id) = le_u32(i)?;
        Ok((i, PacketType::EntityLeave(EntityLeavePacket { entity_id })))
    }

    fn parse_entity_enter<'a, 'b>(
        &'b mut self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        let (i, entity_id) = le_u32(i)?;
        let (i, space_id) = le_u32(i)?;
        let (i, vehicle_id) = le_u32(i)?;
        Ok((
            i,
            PacketType::EntityEnter(EntityEnterPacket {
                entity_id,
                space_id,
                vehicle_id,
            }),
        ))
    }

    fn parse_cruise_state<'a, 'b>(
        &'b mut self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        let (i, key) = le_u32(i)?;
        let (i, value) = le_i32(i)?;
        Ok((i, PacketType::CruiseState(CruiseState { key, value })))
    }

    fn parse_map_packet<'a, 'b>(
        &'b mut self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        std::fs::write("map.bin", i);
        let (i, space_id) = le_u32(i)?;
        let (i, arena_id) = le_i64(i)?;
        let (i, unknown1) = le_u32(i)?;
        let (i, unknown2) = le_u32(i)?;
        let (i, blob) = take(128usize)(i)?;
        let (i, string_size) = le_u32(i)?;
        let (i, map_name) = take(string_size)(i)?;
        let (i, matrix) = take(4usize * 4 * 4)(i)?;
        let (i, unknown) = le_u8(i)?;
        let packet = MapPacket {
            space_id,
            arena_id,
            unknown1,
            unknown2,
            blob,
            // TODO: Use a nom combinator for this (for error handling)
            map_name: std::str::from_utf8(map_name).unwrap(),
            matrix,
            unknown,
        };
        Ok((i, PacketType::Map(packet)))
    }

    fn parse_naked_packet<'a, 'b>(
        &'b mut self,
        packet_type: u32,
        packet: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        /*
        PACKETS_MAPPING = {
            0x0: BasePlayerCreate,
            0x1: CellPlayerCreate,
            0x2: EntityControl,
            0x3: EntityEnter,
            0x4: EntityLeave,
            0x5: EntityCreate,
            # 0x6
            0x7: EntityProperty,
            0x8: EntityMethod,
            0x27: Map,
            0x22: NestedProperty,
            0x0a: Position
        }
        */
        let (i, payload) = match packet_type {
            //0x7 | 0x8 => self.parse_entity_packet(version, packet_type, i)?,
            0x0 => self.parse_base_player_create(packet)?,
            0x1 => self.parse_cell_player_create(packet)?,
            0x3 => self.parse_entity_enter(packet)?,
            0x4 => self.parse_entity_leave(packet)?,
            0x5 => self.parse_entity_create(packet)?,
            0x7 => self.parse_entity_property_packet(packet)?,
            0x8 => self.parse_entity_method_packet(packet)?,
            0xA => self.parse_position_packet(packet)?,
            0x16 => self.parse_version_packet(packet)?,
            0x22 => self.parse_battle_results(packet)?,
            0x23 => self.parse_nested_property_update(packet)?,
            0x25 => self.parse_camera_packet(packet)?, // Note: We suspect that 0x18 is this also
            0x27 => self.parse_camera_mode_packet(packet)?,
            0x28 => self.parse_map_packet(packet)?,
            0x2c => self.parse_player_orientation_packet(packet)?,
            0x2f => self.parse_camera_freelook_packet(packet)?,
            0x32 => self.parse_cruise_state(packet)?,
            _ => self.parse_unknown_packet(packet, packet.len().try_into().unwrap())?,
        };
        Ok((i, payload))
    }

    fn parse_packet<'a, 'b>(&'b mut self, i: &'a [u8]) -> IResult<&'a [u8], Packet<'a, 'b>> {
        let (i, packet_size) = le_u32(i)?;
        let (i, packet_type) = le_u32(i)?;
        let (i, clock) = le_f32(i)?;
        let (remaining, packet_data) = take(packet_size)(i)?;
        let raw = packet_data;
        let (_i, payload) = match self.parse_naked_packet(packet_type, packet_data) {
            Ok(x) => x,
            Err(nom::Err::Failure(Error {
                kind: ErrorKind::UnsupportedReplayVersion(n),
                ..
            })) => {
                return Err(failure_from_kind(ErrorKind::UnsupportedReplayVersion(n)));
            }
            Err(e) => {
                (
                    &packet_data[0..0], // Empty reference
                    PacketType::Invalid(InvalidPacket {
                        message: format!("{:?}", e),
                        raw: packet_data,
                    }),
                )
            }
        };
        // TODO: Add this back
        //assert!(i.len() == 0);
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

    pub fn parse_packets_mut<'a, 'b, P: PacketProcessorMut>(
        &'b mut self,
        i: &'a [u8],
        p: &mut P,
    ) -> Result<(), ErrorKind> {
        let mut i = i;
        while i.len() > 0 {
            let (remaining, packet) = self.parse_packet(i)?;
            i = remaining;
            p.process_mut(packet);
        }
        Ok(())
    }

    pub fn parse_packets<'a, 'b, P: PacketProcessor>(
        &'b mut self,
        i: &'a [u8],
        p: &P,
    ) -> Result<(), ErrorKind> {
        let mut i = i;
        while i.len() > 0 {
            let (remaining, packet) = self.parse_packet(i)?;
            i = remaining;
            p.process(packet);
        }
        Ok(())
    }
}

pub trait PacketProcessor {
    fn process(&self, packet: Packet<'_, '_>);
}
pub trait PacketProcessorMut {
    fn process_mut(&mut self, packet: Packet<'_, '_>);
}
