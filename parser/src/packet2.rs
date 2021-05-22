use log::warn;
use nom::{
    bytes::complete::take, multi::count, number::complete::be_u32, number::complete::be_u8,
    number::complete::le_f32, number::complete::le_u16, number::complete::le_u32,
    number::complete::le_u8,
};
use serde_derive::Serialize;
use std::collections::HashMap;
use std::convert::TryInto;

use crate::error::*;
use crate::rpc::entitydefs::*;
use crate::rpc::typedefs::ArgValue;

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

/*#[derive(Debug, Serialize)]
pub struct ParsedEntityProperty {
    pub property: String,
    pub value: ArgValue,
}*/

#[derive(Debug, Serialize)]
pub struct EntityPropertyPacket<'b> {
    pub entity_id: u32,
    pub property: &'b str,
    pub value: ArgValue<'b>,
}

/*#[derive(Debug, Serialize)]
pub struct ParsedEntityMethodCall {
    pub method: String,
    pub args: Vec<ArgValue>,
}*/

#[derive(Debug, Serialize)]
pub struct EntityMethodPacket<'b> {
    pub entity_id: u32,
    pub method: &'b str,
    pub args: Vec<ArgValue<'b>>,
}

#[derive(Debug, Serialize)]
pub struct EntityCreatePacket<'b> {
    pub entity_id: u32,
    pub entity_type: u16,
    pub vehicle_id: u32,
    pub space_id: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub dir_x: f32,
    pub dir_y: f32,
    pub dir_z: f32,
    pub unknown: u32,
    //pub state: &'a [u8],
    pub props: HashMap<&'b str, crate::rpc::typedefs::ArgValue<'b>>,
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
pub struct InvalidPacket<'a> {
    message: String,
    raw: &'a [u8],
}

#[derive(Debug, Serialize)]
pub struct BasePlayerCreatePacket<'a> {
    pub entity_id: u32,
    pub entity_type: u16,
    pub state: &'a [u8],
}

#[derive(Debug, Serialize)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Serialize)]
pub struct CellPlayerCreatePacket<'a> {
    pub entity_id: u32,
    pub space_id: u32,
    pub unknown: u16,
    pub vehicle_id: u32,
    pub position: Vector3,
    pub direction: Vector3,
    pub value: &'a [u8],
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
pub enum PacketType<'a, 'b> {
    Position(PositionPacket),
    BasePlayerCreate(BasePlayerCreatePacket<'a>),
    CellPlayerCreate(CellPlayerCreatePacket<'a>),
    EntityEnter(EntityEnterPacket),
    EntityLeave(EntityLeavePacket),
    EntityCreate(EntityCreatePacket<'b>),
    EntityProperty(EntityPropertyPacket<'b>),
    EntityMethod(EntityMethodPacket<'b>),
    //Entity(EntityPacket<'a>), // 0x7 and 0x8 are known to be of this type
    //Chat(ChatPacket<'a>),
    //Timing(TimingPacket),
    //ArtilleryHit(ArtilleryHitPacket<'a>),
    //Banner(Banner),
    //DamageReceived(DamageReceivedPacket),
    //Type24(Type24Packet),
    PlayerOrientation(PlayerOrientationPacket),
    //Type8_79(Vec<(u32, u32)>),
    //Setup(SetupPacket),
    //ShipDestroyed(ShipDestroyedPacket),
    //VoiceLine(VoiceLinePacket),
    Unknown(&'a [u8]),

    /// These are packets which we thought we understood, but couldn't parse
    Invalid(InvalidPacket<'a>),
}

#[derive(Debug, Serialize)]
pub struct Packet<'a, 'b> {
    pub packet_size: u32,
    pub packet_type: u32,
    pub clock: f32,
    pub payload: PacketType<'a, 'b>,
    pub raw: &'a [u8],
}

pub struct Parser {
    version: u32,
    specs: Vec<EntitySpec>,

    /// Maps the entity IDs to the entity type
    entities: HashMap<u32, u16>,
}

impl Parser {
    pub fn new(entities: Vec<EntitySpec>) -> Parser {
        Parser {
            version: 0,
            specs: entities,
            entities: HashMap::new(),
        }
    }

    fn parse_entity_property_packet<'a, 'b>(
        &'b self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
        let (i, entity_id) = le_u32(i)?;
        let (i, prop_id) = le_u32(i)?;
        let (i, payload_length) = le_u32(i)?;
        let (i, payload) = take(payload_length)(i)?;

        let entity_type = self.entities.get(&entity_id).unwrap();
        let spec = &self.specs[*entity_type as usize - 1].properties[prop_id as usize];

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

        let entity_type = self.entities.get(&entity_id).unwrap();

        let spec = &self.specs[*entity_type as usize - 1].client_methods[method_id as usize];

        let mut i = payload;
        let mut args = vec![];
        for arg in spec.args.iter() {
            let (new_i, pval) = arg.parse_value(i).unwrap();
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

    fn parse_position_packet<'a, 'b>(
        &'b self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
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

    fn parse_player_orientation_packet<'a, 'b>(
        &'b self,
        i: &'a [u8],
    ) -> IResult<&'a [u8], PacketType<'a, 'b>> {
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
        self.entities.insert(entity_id, entity_type);
        Ok((
            i,
            PacketType::BasePlayerCreate(BasePlayerCreatePacket {
                entity_id,
                entity_type,
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
        let (i, posx) = le_f32(i)?;
        let (i, posy) = le_f32(i)?;
        let (i, posz) = le_f32(i)?;
        let (i, dirx) = le_f32(i)?;
        let (i, diry) = le_f32(i)?;
        let (i, dirz) = le_f32(i)?;
        let (i, unknown) = le_u32(i)?;
        let (_, state) = take(i.len())(i)?;
        if self.entities.contains_key(&entity_id) {
            //println!("DBG: Entity {} got created twice!", entity_id);
        }
        self.entities.insert(entity_id, entity_type);

        let (i, num_props) = le_u8(i)?;
        /*println!(
            "Creating entity type {} with {} props {:?}",
            entity_type, num_props, i
        );*/
        let mut i = i;
        let mut props: HashMap<&str, _> = HashMap::new();
        for _ in 0..num_props {
            let (new_i, prop_id) = le_u8(i)?;
            let spec = &self.specs[entity_type as usize - 1].properties[prop_id as usize];
            //println!("spec {} {}: {:?}", prop_id, new_i.len(), spec.prop_type);
            let (new_i, value) = spec.prop_type.parse_value(new_i).unwrap();
            //println!("{:?}", value);
            i = new_i;
            props.insert(&spec.name, value);
        }
        //println!("{:?}", props);

        Ok((
            i,
            PacketType::EntityCreate(EntityCreatePacket {
                entity_id,
                entity_type,
                vehicle_id,
                space_id,
                position_x: posx,
                position_y: posy,
                position_z: posz,
                dir_x: dirx,
                dir_y: diry,
                dir_z: dirz,
                unknown,
                //state,
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
        //let (i, unknown) = le_u16(i)?;
        let (i, vehicle_id) = le_u32(i)?;
        let (i, posx) = le_f32(i)?;
        let (i, posy) = le_f32(i)?;
        let (i, posz) = le_f32(i)?;
        let (i, dirx) = le_f32(i)?;
        let (i, diry) = le_f32(i)?;
        let (i, dirz) = le_f32(i)?;
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
        let entity_type = self.entities.get(&entity_id).unwrap();
        let spec = &self.specs[*entity_type as usize - 1];
        let mut value = value;
        let mut prop_values = vec![];
        for (idx, property) in spec.internal_properties.iter().enumerate() {
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
                position: Vector3 {
                    x: posx,
                    y: posy,
                    z: posz,
                },
                direction: Vector3 {
                    x: dirx,
                    y: diry,
                    z: dirz,
                },
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

    fn parse_naked_packet<'a, 'b>(
        &'b mut self,
        packet_type: u32,
        i: &'a [u8],
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
        let orig_i = i;
        let (i, payload) = match packet_type {
            //0x7 | 0x8 => self.parse_entity_packet(version, packet_type, i)?,
            0x0 => self.parse_base_player_create(i)?,
            0x1 => self.parse_cell_player_create(i)?,
            0x3 => self.parse_entity_enter(i)?,
            0x4 => self.parse_entity_leave(i)?,
            0x5 => self.parse_entity_create(i)?,
            0x7 => self.parse_entity_property_packet(i)?,
            0x8 => self.parse_entity_method_packet(i)?,
            0xA => self.parse_position_packet(i)?,
            /*0x22 => {
                // Nested property packet?
                println!("{:#?}", i);
                panic!();
            },*/
            /*0x24 => {
                parse_type_24_packet(i)?
            }*/
            0x2b => self.parse_player_orientation_packet(i)?,
            _ => self.parse_unknown_packet(i, i.len().try_into().unwrap())?,
        };
        /*if galil_seiferas::gs_find(orig_i, &[69u8, 69, 80, 76, 69]).is_some() {
            println!("{:#?}", payload);
            println!("{}", orig_i.len());
            //panic!();
        }*/
        Ok((i, payload))
    }

    fn parse_packet<'a, 'b>(&'b mut self, i: &'a [u8]) -> IResult<&'a [u8], Packet<'a, 'b>> {
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
        let (i, payload) = match self.parse_naked_packet(packet_type, i) {
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

    pub fn parse_packets<'a, 'b, P: PacketProcessor>(
        &'b mut self,
        i: &'a [u8],
        p: &mut PacketProcessor,
    ) -> Result<(), ErrorKind> {
        let mut i = i;
        //let mut v = vec![];
        while i.len() > 0 {
            let (remaining, packet) = self.parse_packet(i)?;
            i = remaining;
            //v.push(packet);
            p.process(packet);
        }
        Ok(())
    }
}

pub trait PacketProcessor {
    fn process(&mut self, packet: Packet<'_, '_>);
}
