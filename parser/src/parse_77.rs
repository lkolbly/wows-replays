use log::debug;
use nom::{
    bytes::complete::tag, bytes::complete::take, number::complete::be_u8, number::complete::le_u24,
    number::complete::le_u32,
};
use serde_derive::Serialize;
use std::collections::HashMap;

use crate::error::*;

// 0x71xx & 0x72xx are data identifiers for references
// 0x55 is a length-delimited string (single-byte length)
// 0x68 is a single-byte reference (referencing the above 0x71 & 0x72 tags)
// 0x49 is a 0xA-delimited string
// 0x4a is a 4-byte integer
// 0x4b is... followed by one byte (some sort of framing structure?)
// 0x5d is... followed by nothing (some sort of framing structure?)
// 0x28 is... followed by nothing (some sort of framing structure?)
// 0x86 is... followed by nothing (some sort of framing structure?)
// 0x7d is... followed by nothing (some sort of framing structure?)
// 0x80 is... followed by nothing (some sort of framing structure?)
// 0x88/0x89 are... followed by nothing (boolean true/false?)
#[derive(Debug, PartialEq, Clone)]
enum Type77 {
    /// The packet appears to be divided into sections, of some sort
    StartSection(u8),

    /// The value is the length
    NextSection(u32),

    EndSection,

    /// Data tags tag data to be later referenced by backrefs
    DataTag(u32),
    BackReference(u32),

    String(String),
    NewlineDelimitedString(String),
    StringPair((String, String)),
    U32(u32),

    /// Within the player info object:
    /// 1: Player ID
    /// 5: Clan name
    /// 16: Username
    /// 1c: Equipped equipment (?)
    /// 1d: Ship/hull ID? (1 more than player ID)
    /// 1e: Player ship ID
    /// 1f: Player ship ID (why does this appear twice?)
    ObjectKey(u8),

    Unknown((u8, Vec<u8>)),
}

fn parse_start_section(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x80])(i)?;
    let (i, x) = be_u8(i)?;
    assert!(x == 2); // What does it mean if x != 2?
    Ok((i, Type77::StartSection(x)))
}

fn parse_next_section(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0xff])(i)?;
    let (i, len) = le_u24(i)?;
    Ok((i, Type77::NextSection(len)))
}

fn parse_end_section(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x2e])(i)?;
    Ok((i, Type77::EndSection))
}

fn parse_backref_u8(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x68])(i)?;
    let (i, key) = be_u8(i)?;
    Ok((i, Type77::BackReference(key as u32)))
}

fn parse_backref_u32(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x6a])(i)?;
    let (i, key) = le_u32(i)?;
    Ok((i, Type77::BackReference(key)))
}

fn parse_backref(i: &[u8]) -> IResult<&[u8], Type77> {
    let mut parser = nom::branch::alt((parse_backref_u8, parse_backref_u32));
    parser(i)
}

fn parse_object_key(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x4b])(i)?;
    let (i, key) = be_u8(i)?;
    Ok((i, Type77::ObjectKey(key)))
}

fn parse_77_length_delimited_string(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x55])(i)?;
    let (i, l) = be_u8(i)?;
    let (i, s) = take(l)(i)?;
    Ok((
        i,
        Type77::String(std::str::from_utf8(s).unwrap().to_string()),
    ))
}

fn parse_77_length_delimited_string_58(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x58])(i)?;
    let (i, l) = le_u32(i)?;
    let (i, s) = take(l)(i)?;
    Ok((
        i,
        Type77::String(std::str::from_utf8(s).unwrap().to_string()),
    ))
}

fn parse_77_newline_delimited_string(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x49])(i)?;
    let search: &[u8] = &[0xa];
    let (i, s) = nom::bytes::complete::take_until(search)(i)?;
    let (i, _) = tag([0xa])(i)?;
    Ok((
        i,
        Type77::NewlineDelimitedString(std::str::from_utf8(s).unwrap().to_string()),
    ))
}

// This is just... two newline delimited strings together?
fn parse_77_newline_delimited_string_63(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x63])(i)?;
    let search: &[u8] = &[0xa];
    let (i, s) = nom::bytes::complete::take_until(search)(i)?;
    let (i, _) = tag([0xa])(i)?;
    let (i, s2) = nom::bytes::complete::take_until(search)(i)?;
    let (i, _) = tag([0xa])(i)?;
    Ok((
        i,
        Type77::StringPair((
            std::str::from_utf8(s).unwrap().to_string(),
            std::str::from_utf8(s2).unwrap().to_string(),
        )),
    ))
}

fn parse_77_int(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x4a])(i)?;
    let (i, x) = le_u32(i)?;
    Ok((i, Type77::U32(x)))
}

fn parse_77_unknown(tag_value: u8, count: usize) -> Box<dyn Fn(&[u8]) -> IResult<&[u8], Type77>> {
    Box::new(move |i| {
        let (i, x) = tag([tag_value])(i)?;
        let (i, y) = take(count)(i)?;
        Ok((i, Type77::Unknown((x[0], y.to_vec()))))
    })
}

/*fn parse_77_7d(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, x) = tag([0x7d])(i)?;
    Ok((i, Type77::Unknown(x)))
}*/

fn parse_77_71(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x71])(i)?;
    let (i, x) = be_u8(i)?;
    Ok((i, Type77::DataTag(x as u32)))
}

fn parse_77_72(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x72])(i)?;
    let (i, x) = le_u32(i)?;
    Ok((i, Type77::DataTag(x)))
}

fn parse_77_datum(i: &[u8]) -> IResult<&[u8], Type77> {
    let framing_parser = nom::branch::alt((
        // Whatever the data-counting-tagging mechanism is
        parse_77_71,
        parse_77_72,
        // These are some sort of framing markers, I think
        //parse_77_unknown(0x80, 1), // e.g. 0x80 0x02
        parse_77_unknown(0x7d, 0),
        parse_77_unknown(0x28, 0),
        //parse_77_unknown(0x4b, 1), // Second byte seems to count and be nested
        parse_77_unknown(0x5d, 0),
        parse_77_unknown(0x65, 0), // Maybe "empty string" or "null"?
        parse_77_unknown(0x86, 0),
        // This is a single-byte backreference? (reference the data specified by the tag)
        //parse_77_unknown(0x68, 1),
        //parse_77_unknown(0x6a, 4), // 4-byte backreference?
    ));
    let datatypes = nom::branch::alt((
        // These datatypes are pretty well-known
        parse_77_length_delimited_string,
        parse_77_length_delimited_string_58,
        parse_77_newline_delimited_string,
        parse_77_newline_delimited_string_63,
        //parse_77_unknown(0x4a, 4), // This is a 4-byte integer
        parse_77_int,
        parse_object_key,
        parse_backref,
        parse_start_section,
        parse_next_section,
        parse_end_section,
    ));
    let (new_i, x) = nom::branch::alt((
        framing_parser,
        datatypes,
        // These are really super unknown, I'm just parsing enough to get past them
        parse_77_unknown(0x75, 0), // This is especially confusing - start of string? Start of array? End of array?
        parse_77_unknown(0x73, 0), // The "s" in "usb"
        parse_77_unknown(0x62, 0), // The "b" in "usb"
        parse_77_unknown(0x4d, 2),
        parse_77_unknown(0x4e, 0),
        parse_77_unknown(0x29, 0),
        //parse_77_unknown(0x2e, 4), // Note: This isn't quite right, it's eating too many symbols at the end of the data
        parse_77_unknown(0x81, 0),
        parse_77_unknown(0x88, 0),
        parse_77_unknown(0x89, 0),
        parse_77_unknown(0x61, 0),
        parse_77_unknown(0x26, 3),
        // These appear in similar-looking situations
        //parse_77_unknown(0x1a, 3),
        //parse_77_unknown(0x51, 3),

        // Interestingly, these appear in similar-looking situations
        //parse_77_unknown(0x21, 3),
        //parse_77_unknown(0x24, 3),

        // This one probably means I screwed up somewhere else
        //parse_77_unknown(0x00, 0),
    ))(i)?;
    Ok((new_i, x))
}

fn parse_section2_key(i: &[u8]) -> IResult<&[u8], (u8, Vec<Type77>, HashMap<u32, Type77>)> {
    let (i, obj_key) = parse_object_key(i)?;
    let obj_key = match obj_key {
        Type77::ObjectKey(n) => n,
        _ => {
            panic!("Got unexpected thing for ObjectKey");
        }
    };
    //println!("Got key #{:?}", obj_key);
    let (i, (data, _)) = nom::multi::many_till(parse_77_datum, tag([0x86]))(i)?;
    //println!("Got {} elements in key", data.len());

    let mut data = data;

    // Key #2 for some reason has an extra 0x86 embedded in it
    let (i, extra_data) = if obj_key == 2 && data.len() > 2 {
        let (i, (x, _)) = nom::multi::many_till(parse_77_datum, tag([0x86]))(i)?;
        //println!("Got {} more elements", x.len());
        (i, x)
    } else {
        (i, vec![])
    };
    let mut extra_data = extra_data;
    data.append(&mut extra_data);

    // Perhaps a data tag
    let (i, _) = match nom::branch::alt((parse_77_71, parse_77_72))(i) {
        Ok((i, datatag)) => (i, Some(datatag)),
        Err(_) => (i, None),
    };

    // Extract all of the data tags from the data
    let mut tagged_data = HashMap::new();
    let mut stripped_data = vec![];
    for i in 0..data.len() {
        match data[i] {
            Type77::DataTag(tag) => match data[i - 1] {
                Type77::NewlineDelimitedString(_)
                | Type77::U32(_)
                | Type77::String(_)
                | Type77::StringPair(_)
                | Type77::Unknown(_) => {
                    tagged_data.insert(tag, data[i - 1].clone());
                }
                _ => {}
            },
            _ => {
                stripped_data.push(data[i].clone());
            }
        }
    }

    //println!("Data: {:x?}", data);

    //let (i, end_obj) = tag([0x86])(i)?;

    Ok((i, (obj_key, stripped_data, tagged_data)))
}

fn parse_section2_array(
    i: &[u8],
) -> IResult<&[u8], (HashMap<u8, Vec<Type77>>, HashMap<u32, Type77>)> {
    let (i, _) = tag([0x28])(i)?;
    let (i, (keys, _)) = nom::multi::many_till(
        parse_section2_key,
        nom::branch::alt((
            tag([0x65, 0x5d]),
            tag([0x65, 0x65]), // This is for the last one
        )),
    )(i)?;

    // Perhaps a data tag
    let (i, _) = match nom::branch::alt((parse_77_71, parse_77_72))(i) {
        Ok((i, datatag)) => (i, Some(datatag)),
        Err(_) => (i, None),
    };

    // Merge all the data tags, build an actual hashmap
    let mut tagged_data = HashMap::new();
    let mut keys = keys;
    let mut data = HashMap::new();
    for (idx, values, mut tagged) in keys.drain(..) {
        data.insert(idx, values);
        for (k, v) in tagged.drain() {
            assert!(!tagged_data.contains_key(&k));
            tagged_data.insert(k, v);
        }
    }

    //let (i, _) = tag([0x65, 0x5d])(i)?;
    //println!("Found {} keys", keys.len());
    Ok((i, (data, tagged_data)))
}

#[derive(Debug, Serialize)]
pub struct SetupPlayerInfo {
    pub username: String,
    pub clan: String,

    /// The ID used to reference the ship in this game
    pub shipid: u32,

    /// The ID used to reference the player/camera (?) in this game
    pub playerid: u32,

    /// The ID of the ship type the player is using
    pub shiptypeid: u32,
}

fn parse_section2(i: &[u8]) -> IResult<&[u8], Vec<SetupPlayerInfo>> {
    //hexdump::hexdump(&i[0..20]);
    let (i, _) = tag([0x5d, 0x71, 0x01, 0x28, 0x5d, 0x71, 0x02])(i)?;
    //println!("Got tag");
    let (i, players) = nom::multi::many1(parse_section2_array)(i)?;

    // Merge all the tagged data
    let mut tagged_data = HashMap::new();
    for (_, players_tags) in players.iter() {
        for (k, v) in players_tags.iter() {
            tagged_data.insert(k.clone(), v.clone());
        }
    }

    // Substitute backrefs
    let mut players = players;
    for (player, _) in players.iter_mut() {
        for (_, values) in player.iter_mut() {
            for v in values.iter_mut() {
                match v {
                    Type77::BackReference(refno) => {
                        *v = tagged_data
                            .get(refno)
                            .expect(&format!("Could not find data tag for refno 0x{:x}", refno))
                            .clone()
                    }
                    _ => {}
                }
            }
        }
    }

    debug!("Found {} players", players.len());
    Ok((
        i,
        players
            .drain(..)
            .map(|(player, _)| {
                debug!("Player data:");
                for i in 0..34 {
                    debug!(" - {}: {:x?}", i, player.get(&i).unwrap());
                }
                let username = match &player.get(&22).expect("Couldn't find username field")[0] {
                    Type77::String(s) => s.clone(),
                    _ => {
                        panic!("Username was not a string!");
                    }
                };
                let clan = match &player.get(&5).expect("Couldn't find clanname field")[0] {
                    Type77::String(s) => s.clone(),
                    _ => {
                        panic!("Clanname was not a string");
                    }
                };
                let shipid = match &player.get(&29).expect("Couldn't find shipid field")[0] {
                    Type77::U32(n) => *n,
                    _ => {
                        panic!("Ship ID was not a U32");
                    }
                };
                let playerid = match &player.get(&1).expect("Couldn't find playerid field")[0] {
                    Type77::U32(n) => *n,
                    _ => {
                        panic!("Player ID was not a U32");
                    }
                };
                let shiptypeid = match &player.get(&30).expect("Couldn't find shiptypeid field")[0]
                {
                    Type77::NewlineDelimitedString(s) => s.clone(),
                    _ => {
                        panic!("Shiptypeid was not a string");
                    }
                };
                let shiptypeid_alt =
                    match &player.get(&31).expect("Couldn't find shiptypeid field")[0] {
                        Type77::NewlineDelimitedString(s) => s.clone(),
                        _ => {
                            panic!("Shiptypeid was not a string");
                        }
                    };
                // Figure out where these are different
                //assert!(shiptypeid == shiptypeid_alt);
                let shiptypeid = shiptypeid
                    .parse::<u32>()
                    .expect("Could not parse shiptypeid field");
                SetupPlayerInfo {
                    username,
                    clan,
                    shipid,
                    playerid,
                    shiptypeid,
                }
            })
            .collect(),
    ))
}

fn parse_sections(i: &[u8]) -> IResult<&[u8], Vec<SetupPlayerInfo>> {
    let (i, _) = parse_start_section(i)?;
    //println!("Got start section");
    let (i, _) = nom::multi::many_till(parse_77_datum, parse_end_section)(i)?;
    //println!("Got some data");
    let (i, _) = parse_next_section(i)?;
    //println!("Got next section");
    let (i, _) = parse_start_section(i)?;
    //println!("Got another start section");
    let (i, players) = parse_section2(i)?;
    /*for player in players.iter() {
        debug!("Found player: {:#?}", player);
    }
    println!("Got section2");*/
    let (i, _) = parse_end_section(i)?;
    Ok((i, players))
}

#[derive(Debug, Serialize)]
pub struct SetupPacket {
    pub players: Vec<SetupPlayerInfo>,
}

pub fn parse_77(i: &[u8]) -> IResult<&[u8], SetupPacket> {
    let (i, start) = take(10u32)(i)?;
    debug!("Got {} bytes of start data: {:?}", start.len(), start);

    let (i, players) = parse_sections(i)?;

    let (i, end) = take(i.len())(i)?;
    debug!("Got {} bytes of ending data: {:?}", end.len(), end);

    Ok((i, SetupPacket { players }))
}
