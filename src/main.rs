use std::io::{Read, Write};
use nom::{bytes::complete::take, bytes::complete::tag, named, do_parse, take, tag, number::complete::be_u16, number::complete::le_u16, number::complete::be_u8, alt, cond, number::complete::be_u24, char, opt, one_of, take_while, length_data, many1, complete, number::complete::le_u32, number::complete::le_f32, multi::many0, number::complete::be_u32};
use serde_derive::{Deserialize, Serialize};
use thiserror::Error;
use std::collections::HashMap;
use std::convert::TryInto;
use crypto::symmetriccipher::BlockDecryptor;
use plotters::prelude::*;

#[derive(Debug, Deserialize, Serialize)]
struct VehicleInfoMeta<'a> {
    shipId: u64,
    relation: u32,
    id: u64, // Account ID?
    name: &'a str,
}

#[derive(Debug, Deserialize, Serialize)]
struct ReplayMeta<'a> {
    matchGroup: &'a str,
    gameMode: u32,
    clientVersionFromExe: &'a str,
    scenarioUiCategoryId: u32,
    mapDisplayName: &'a str,
    mapId: u32,
    clientVersionFromXml: &'a str,
    weatherParams: HashMap<&'a str, Vec<&'a str>>,
    //mapBorder: Option<...>,
    duration: u32,
    gameLogic: &'a str,
    name: &'a str,
    scenario: &'a str,
    playerID: u32,
    vehicles: Vec<VehicleInfoMeta<'a>>,
    playersPerTeam: u32,
    dateTime: &'a str,
    mapName: &'a str,
    playerName: &'a str,
    scenarioConfigId: u32,
    teamsCount: u32,
    logic: &'a str,
    playerVehicle: &'a str,
    battleDuration: u32,
}

#[derive(Debug)]
struct Replay<'a> {
    meta: ReplayMeta<'a>,
    uncompressed_size: u32,
    compressed_stream: &'a [u8],
}

#[derive(Debug)]
struct Error<I: std::fmt::Debug> {
    pub kind: ErrorKind<I>,
    backtrace: Vec<ErrorKind<I>>,
}

#[derive(Error, Debug)]
enum ErrorKind<I: std::fmt::Debug> {
    #[error("Nom error")]
    Nom {
        err: nom::error::ErrorKind,
        input: I,
    },
    #[error("Error parsing json")]
    Serde {
        #[from]
        err: serde_json::Error,
    },
    #[error("Error interpreting UTF-8 string")]
    Utf8Error {
        #[from]
        err: std::str::Utf8Error,
    },
}

impl<I: std::fmt::Debug> nom::error::ParseError<I> for Error<I> {
    fn from_error_kind(input: I, kind: nom::error::ErrorKind) -> Self {
        Self {
            kind: ErrorKind::Nom { err: kind, input: input },
            backtrace: Vec::new()
        }
    }

    fn append(input: I, kind: nom::error::ErrorKind, mut other: Self) -> Self {
        other.backtrace.push(Self::from_error_kind(input, kind).kind);
        other
    }
}

impl<I: std::fmt::Debug> std::convert::From<std::str::Utf8Error> for Error<I> {
    fn from(x: std::str::Utf8Error) -> Error<I> {
        Error {
            kind: x.into(),
            backtrace: vec!(),
        }
    }
}

impl<I: std::fmt::Debug> std::convert::From<serde_json::Error> for Error<I> {
    fn from(x: serde_json::Error) -> Error<I> {
        Error {
            kind: x.into(),
            backtrace: vec!(),
        }
    }
}

type IResult<I, T> = nom::IResult<I, T, Error<I>>;

#[derive(Debug)]
struct PositionPacket {
    pid: u32,
    //clock: f32,
    x: f32,
    y: f32,
    z: f32,
    rot_x: u32,
    rot_y: u32,
    rot_z: u32,
    a: f32, // These three appear to be velocity in x,y,z (perhaps local? Forward/back velocity and side-to-side drift?)
    b: f32,
    c: f32,
    extra: u8,
    //raw: &'a [u8],
}

#[derive(Debug)]
struct Type8Packet<'a> {
    unknown: u32,
    subtype: u32,
    payload: &'a [u8],
}

#[derive(Debug)]
struct ChatPacket<'a> {
    sender_id: u32,
    audience: &'a str,
    message: &'a str,
}

#[derive(Debug)]
enum PacketType<'a> {
    Position(PositionPacket),
    Type8(Type8Packet<'a>),
    Chat(ChatPacket<'a>),
    Unknown(&'a [u8]),
}

#[derive(Debug)]
struct Packet<'a> {
    packet_size: u32,
    packet_type: u32,
    clock: f32,
    payload: PacketType<'a>,
    raw: &'a [u8],
}

fn parse_chat_packet(i: &[u8]) -> IResult<&[u8], PacketType> {
    //hexdump::hexdump(i);
    let (i, sender) = le_u32(i)?;
    let (i, audience_len) = be_u8(i)?;
    let (i, audience) = take(audience_len)(i)?;
    let (i, message_len) = be_u8(i)?;
    let (i, message) = take(message_len)(i)?;
    Ok((
        i,
        PacketType::Chat(ChatPacket{
            sender_id: sender,
            audience: std::str::from_utf8(audience).unwrap(),
            message: std::str::from_utf8(message).unwrap(),
        })
    ))
}

fn parse_type8(i: &[u8]) -> IResult<&[u8], PacketType> {
    let (i, unknown) = le_u32(i)?;
    let (i, subtype) = le_u32(i)?; // Probably?
    let (i, payload_length) = le_u32(i)?;
    let (i, payload) = take(payload_length)(i)?;
    //println!("Parsing type 8 subtype=0x{:x}", subtype);
    if subtype == 0x76 {
        Ok((
            i,
            parse_chat_packet(payload)?.1
        ))
    } else {
        Ok((
            i,
            PacketType::Type8(Type8Packet{
                unknown: unknown,
                subtype: subtype,
                payload: payload,
            })
        ))
    }
}

fn parse_position_packet(i: &[u8]) -> IResult<&[u8], PacketType> {
    //let (i, packet_size) = le_u32(i)?;
    //let (i, _) = tag([0xA, 0, 0, 0])(i)?;
    //let (i, clock) = le_f32(i)?;
    //let (remaining, i) = take(packet_size)(i)?;
    //let raw = i;
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
    /*let (i, packet_size) = le_u32(i)?;
    let (i, packet_type) = le_u32(i)?;
    let (i, clock) = le_f32(i)?;
    let (i, contents) = take(packet_size)(i)?;*/
    let (i, contents) = take(payload_size)(i)?;
    Ok((
        i,
        PacketType::Unknown(contents)
    ))
}

fn parse_packet(i: &[u8]) -> IResult<&[u8], Packet> {
    let (i, packet_size) = le_u32(i)?;
    let (i, packet_type) = le_u32(i)?;//tag([0xA, 0, 0, 0])(i)?;
    let (i, clock) = le_f32(i)?;
    let (remaining, i) = take(packet_size)(i)?;
    let raw = i;
    let (i, payload) = match packet_type {
        0x8 => {
            parse_type8(i)?
        }
        0xA => {
            parse_position_packet(i)?
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
    /*alt!(
        i,
        parse_position_packet |
        parse_unknown_packet
    )*/
}

fn parse_packets(i: &[u8]) -> IResult<&[u8], Vec<Packet>> {
    many0(parse_packet)(i)
}

/*named!(parse_packet<Packet>,
       do_parse!(
           packet_size: le_u32 >>
               packet_type: le_u32 >>
               clock: le_f32 >>
               contents: take!(packet_size) >>
               (Packet::Unknown((packet_type, clock, contents)))
       )
);*/

fn decode_meta(meta: &[u8]) -> Result<ReplayMeta, Error<&[u8]>> {
    let meta = std::str::from_utf8(meta)?;
    let meta: ReplayMeta = serde_json::from_str(meta)?;
    Ok(meta)
}

fn parse_meta(i: &[u8]) -> IResult<&[u8], ReplayMeta> {
    let (i, meta_len) = le_u32(i)?;
    let (i, meta) = take(meta_len)(i)?;
    let meta = match decode_meta(meta) {
        Ok(x) => { x }
        Err(e) => {
            return Err(nom::Err::Error(e.into()));
        }
    };
    Ok((i, meta))
}

fn replay_format(i: &[u8]) -> IResult<&[u8], Replay> {
    let (i, unknown) = take(8usize)(i)?;
    let (i, meta) = parse_meta(i)?;
    let (i, uncompressed_size) = le_u32(i)?;
    let (i, stream_size) = le_u32(i)?;
    Ok((i, Replay{
        meta: meta,
        uncompressed_size: uncompressed_size,
        compressed_stream: unknown,
    }))
}

/*named!(replay_format<Replay>,
       do_parse!(
           unknown: take!(8) >>
               meta: parse_meta >>
               //meta_len: le_u32 >>
               //meta: take!(meta_len) >>
               uncompressed_size: le_u32 >> // Probably?
               stream_size: le_u32 >>
               //compressed_stream: take!(stream_size) >>
               (Replay{
                   meta: meta,
                   uncompressed_size: uncompressed_size,
                   compressed_stream: unknown,//compressed_stream,
               })
       )
);*/

/*unsafe extern "C" fn dummy_fn(_: *mut std::ffi::c_void, _: *mut std::ffi::c_void) {
    //
}

unsafe extern "C" fn dummy_fn2(_: *mut std::ffi::c_void, items: u32, size: u32) -> *mut std::ffi::c_void {
    //std::ptr::null_mut()
    //let b = Box::new([0u8; items * size]);
    //Box::into_raw(b)
    std::alloc::alloc(std::alloc::Layout::from_size_align(size as usize * items as usize, 1).unwrap()) as *mut std::ffi::c_void
}*/

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
#[derive(Debug)]
enum Type77<'a> {
    DataTag(u32),
    String(&'a str),
    StringPair((&'a str, &'a str)),
    U32(u32),
    Unknown((u8, &'a [u8])),
}

fn parse_77_length_delimited_string(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x55])(i)?;
    let (i, l) = be_u8(i)?;
    let (i, s) = take(l)(i)?;
    Ok((i, Type77::String(std::str::from_utf8(s).unwrap())))
}

fn parse_77_length_delimited_string_58(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x58])(i)?;
    let (i, l) = le_u32(i)?;
    let (i, s) = take(l)(i)?;
    Ok((i, Type77::String(std::str::from_utf8(s).unwrap())))
}

fn parse_77_newline_delimited_string(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x49])(i)?;
    let search: &[u8] = &[0xa];
    let (i, s) = nom::bytes::complete::take_until(search)(i)?;
    let (i, _) = tag([0xa])(i)?;
    Ok((i, Type77::String(std::str::from_utf8(s).unwrap())))
}

// This is just... two newline delimited strings together?
fn parse_77_newline_delimited_string_63(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, _) = tag([0x63])(i)?;
    let search: &[u8] = &[0xa];
    let (i, s) = nom::bytes::complete::take_until(search)(i)?;
    let (i, _) = tag([0xa])(i)?;
    let (i, s2) = nom::bytes::complete::take_until(search)(i)?;
    let (i, _) = tag([0xa])(i)?;
    Ok((i, Type77::StringPair((
        std::str::from_utf8(s).unwrap(),
        std::str::from_utf8(s2).unwrap(),
    ))))
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
        Ok((i, Type77::Unknown((x[0], y))))
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

/*fn parse_80_02(i: &[u8]) -> IResult<&[u8], Type77> {
    let (i, x) = tag([0x80, 0x02])(i)?;
    Ok((i, Type77::Unknown(x)))
}*/

fn parse_77(i: &[u8]) -> IResult<&[u8], ()> {
    let orig_len = i.len();
    let (i, start) = take(10u32)(i)?;
    /*let (i, v) = many1!(i, alt!(
        parse_77_71 |
        parse_77_72
    ))?;*/
    let mut v = vec!();
    let mut i = i;
    loop {
        let framing_parser = nom::branch::alt((
            // Whatever the data-counting-tagging mechanism is
            parse_77_71,
            parse_77_72,

            // These are some sort of framing markers, I think
            parse_77_unknown(0x80, 1), // e.g. 0x80 0x02
            parse_77_unknown(0x7d, 0),
            parse_77_unknown(0x28, 0),
            parse_77_unknown(0x4b, 1), // Second byte seems to count and be nested
            parse_77_unknown(0x5d, 0),
            parse_77_unknown(0x65, 0), // Maybe "empty string" or "null"?
            parse_77_unknown(0x86, 0),

            // This is a single-byte backreference? (reference the data specified by the tag)
            parse_77_unknown(0x68, 1),
            parse_77_unknown(0x6a, 4), // 4-byte backreference?
        ));
        let datatypes = nom::branch::alt((
            // These datatypes are pretty well-known
            parse_77_length_delimited_string,
            parse_77_length_delimited_string_58,
            parse_77_newline_delimited_string,
            parse_77_newline_delimited_string_63,
            //parse_77_unknown(0x4a, 4), // This is a 4-byte integer
            parse_77_int,
        ));
        let (new_i, x) = match nom::branch::alt((
            framing_parser,
            datatypes,

            // These are really super unknown, I'm just parsing enough to get past them
            parse_77_unknown(0x75, 0), // This is especially confusing - start of string? Start of array? End of array?
            parse_77_unknown(0x73, 0), // The "s" in "usb"
            parse_77_unknown(0x62, 0), // The "b" in "usb"

            parse_77_unknown(0x4d, 2),
            parse_77_unknown(0x4e, 0),
            parse_77_unknown(0x29, 0),
            parse_77_unknown(0x2e, 4), // Note: This isn't quite right, it's eating too many symbols at the end of the data
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
        ))(i) {
            Ok(x) => { x }
            Err(_) => { break; }
        };
        v.push(x);
        i = new_i;
    }
    let mut indent = 3;
    for x in v.iter() {
        let mut indents = String::new();
        //println!("{}", indent);
        if indent < 0 {
            indents.push_str("BAD");
        }
        for i in 0..indent {
            indents.push_str("  ");
        }
        println!("{}{:x?}", indents, x);
        match x {
            Type77::Unknown((0x28, _)) => { indent += 1; }
            Type77::Unknown((0x29, _)) => { indent += 1; }
            Type77::Unknown((0x61, _)) => { indent += 1; }
            Type77::Unknown((0x5d, _)) => { indent -= 1; }
            Type77::Unknown((0x7d, _)) => { indent -= 1; }
            _ => {}
        }
        //if indent < 0 { indent = 0; }
    }
    println!("Started with {} bytes", orig_len);
    println!("Start data: {:x?}", start);
    println!("Got {} packets, but remaining {} bytes start with:", v.len(), i.len());
    if i.len() < 160 {
        hexdump::hexdump(i);
    } else {
        hexdump::hexdump(&i[0..10*16]);
    }
    Ok((i, ()))
}

/*fn parse_77(payload: &[u8]) {
    // Find every "7x nn" for xnn incrementing (starting at 71 01)
    let mut offset = 0;
    let mut count: u32 = 1;
    let mut previous_offset = 0;
    loop {
        if offset + 1 > payload.len() {
            return;
        }
        if payload[offset] == (if count > 255 { 0x72 } else { 0x71 }) as u8 && payload[offset + 1] == (count % 256) as u8 {
            let sub = &payload[previous_offset..offset];

            let (pid, subtype, payload) = if sub[0] == 0x71 {
                (
                    sub[1] as u32,
                    sub[2],
                    &sub[3..],
                )
            } else {
                (
                    u32::from_le_bytes(sub[1..5].try_into().unwrap()),
                    sub[5],
                    &sub[6..],
                )
            };

            if subtype == 0x55 {
                let l = payload[0] as usize;
                let s = std::str::from_utf8(&payload[1..1+l]).unwrap();
                //println!("{:x?}", sub);
                //println!("{} {}", sub.len(), l);
                //println!("{:x?}", payload);
                assert!(payload.len() == 1 + l);
                println!("0x{:x} 0x{:x}: \"{}\"", pid, subtype, s);
            } else {
                println!("0x{:x} 0x{:x}: {:x?}", pid, subtype, payload);
            }
            count += 1;
            previous_offset = offset;
        }
        offset += 1;
    }
}*/

fn parse_replay(replay: &std::path::PathBuf) {
    let mut f = std::fs::File::open(replay).unwrap();
    let mut contents = vec!();
    f.read_to_end(&mut contents).unwrap();

    //println!("{:x}", contents[0]);

    let (remaining, result) = replay_format(&contents).unwrap();
    //let meta_str = std::str::from_utf8(result.meta).unwrap();
    //println!("{:?}", meta_str);

    let mut f = std::fs::File::create("meta.json").unwrap();
    f.write_all(serde_json::to_string(&result.meta).unwrap().as_bytes()).unwrap();

    // Decrypt
    let key = [0x29, 0xB7, 0xC9, 0x09, 0x38, 0x3F, 0x84, 0x88, 0xFA, 0x98, 0xEC, 0x4E, 0x13, 0x19, 0x79, 0xFB];
    let blowfish = crypto::blowfish::Blowfish::new(&key);
    assert!(blowfish.block_size() == 8);
    let encrypted = remaining;//result.compressed_stream
    let mut decrypted = vec!();
    decrypted.resize(encrypted.len(), 0u8);
    let num_blocks = encrypted.len() / blowfish.block_size();
    let mut previous = [0; 8]; // 8 == block size
    for i in 0..num_blocks {
        let offset = i * blowfish.block_size();
        blowfish.decrypt_block(
            &encrypted[offset..offset+blowfish.block_size()],
            &mut decrypted[offset..offset+blowfish.block_size()]
        );
        for j in 0..8 {
            decrypted[offset + j] = decrypted[offset + j] ^ previous[j];
            previous[j] = decrypted[offset + j];
        }
    }

    println!("---------------------------------------------------------------");
    //println!("Uncompressed size (?): {} or 0x{:x} bytes, {}x bigger", result.uncompressed_size, result.uncompressed_size, result.uncompressed_size as f64 / result.compressed_stream.len() as f64);
    //println!("Amount remaining: {} or 0x{:x} bytes", remaining.len(), remaining.len());
    //hexdump::hexdump(&decrypted[0..16*10]);
    //hexdump::hexdump(remaining);
    //println!("0x{:x} 0x{:x} 0x{:x}", result.compressed_stream[0], result.compressed_stream[1], result.meta.mapId);
    /*let mut hi = hexdump::hexdump_iter(result.compressed_stream);
    for i in 0..40 {
        println!("{}", hi.next().unwrap());
    }*/

    //println!("{:#?}", result.meta);

    /*let mut dec = lzw::Decoder::new(lzw::MsbReader::new(), 8);
    //let mut bytes = [0; 256];
    dec.decode_bytes(&result.compressed_stream[0..2]).unwrap();*/

    let mut deflater = flate2::read::ZlibDecoder::new(&decrypted[..]);
    let mut contents = vec!();
    deflater.read_to_end(&mut contents).unwrap();
    //hexdump::hexdump(&contents[0..16*10]);
    //println!("{} bytes at the end", contents.len());

    let mut f = std::fs::File::create("packets.bin").unwrap();
    f.write_all(&contents).unwrap();

    // Copy the WoT algorithm
    /*let mut strm = libz_sys::z_stream{
        next_in: decrypted.as_mut_ptr(),
        avail_in: decrypted.len().try_into().unwrap(),
        total_in: 0,
        next_out: std::ptr::null_mut(),
        avail_out: 0,
        total_out: 0,
        msg: std::ptr::null_mut(),
        state: std::ptr::null_mut(),
        zalloc: dummy_fn2,//std::ptr::null_mut(),
        zfree: dummy_fn,//std::ptr::null_mut(),
        opaque: std::ptr::null_mut(),
        data_type: 0,
        adler: 0,
        reserved: 0,
    };
    let cstr = std::ffi::CString::new("1.2.101").unwrap();
    let replay = unsafe {
        let ret = libz_sys::inflateInit_(&mut strm, cstr.as_c_str().as_ptr(), std::mem::size_of::<libz_sys::z_stream>() as i32);
        println!("ret = {} msg = {:?}", ret, strm.msg);

        let mut replay = vec!();
        let chunk = 10*1024*1024;
        loop {
            let mut out = vec!();
            out.resize(chunk, 0u8);

            strm.avail_out = chunk as u32;
            strm.next_out = (&mut out).as_mut_ptr();
            let ret = libz_sys::inflate(&mut strm, libz_sys::Z_NO_FLUSH);
            println!("ret = {}", ret);
            if ret == libz_sys::Z_DATA_ERROR {
                libz_sys::inflateEnd(&mut strm);
                //panic!("fadfs");
            }
            let have = chunk - strm.avail_out as usize;
            println!("Got {} bytes", have);
            replay.append(&mut out);
            if strm.avail_out != 0 {
                break;
            }
        }

        replay
    };
    hexdump::hexdump(&replay[0..16*10]);*/

    let root = BitMapBackend::new("test.png", (2048, 2048)).into_drawing_area();
    root.fill(&BLACK).unwrap();

    let areas = root.split_by_breakpoints([2000], [80]);

    /*let mut x_hist_ctx = ChartBuilder::on(&areas[0])
        .y_label_area_size(40)
        .build_ranged(0u32..100u32, 0f64..0.5f64)?;
    let mut y_hist_ctx = ChartBuilder::on(&areas[3])
        .x_label_area_size(40)
        .build_ranged(0f64..0.5f64, 0..100u32)?;*/
    let mut scatter_ctx = ChartBuilder::on(&areas[2])
        .x_label_area_size(40)
        .y_label_area_size(40)
        .build_ranged(-700.0..700.0, -700.0..700.0).unwrap();

    // Parse packets
    let (remaining, packets) = parse_packets(&contents).unwrap();
    let mut points = HashMap::new();
    for packet in packets.iter() {
        match packet {
            Packet { clock: clock, payload: PacketType::Position(p), .. } => {
                if !points.contains_key(&p.pid) {
                    points.insert(p.pid, vec!());
                }
                points.get_mut(&p.pid).unwrap().push((p.x as f64, p.z as f64));
                if p.pid == 0x8f22c {
                    //println!("Got pos: {} 0x{:x} {} {} {} - {} {} {} - {} {} {}", clock, p.pid, p.x, p.y, p.z, p.rot_x, p.rot_y, p.rot_z, p.a, p.b, p.c);
                    //println!("{:x?}", p.raw);
                    //points.push((p.x as f64, p.z as f64));
                }
                /*let pid = u32::from_le_bytes(data[0..4].try_into().unwrap());
                let x = f32::from_le_bytes(data[8..12].try_into().unwrap());
                if pid == 0x8f22c {
                    println!("Packet 10: {} 0x{:x} {:x?}", clock, pid, data);
                }*/
                //println!("Got packet: {:?}", packet);
            }
            Packet { clock, payload: PacketType::Type8(p), .. } => {
                if p.subtype == 0x77 {
                    println!("{}: Got {}-byte 0x8 packet subtype=0x{:x}:", clock, p.payload.len(), p.subtype);
                    //hexdump::hexdump(p.payload);
                } else if p.subtype == 0x76 {
                    println!("{}: Got chat packet!", clock);
                }
            }
            Packet { clock, payload: PacketType::Chat(p), .. } => {
                println!("{}: Got chat packet: audience='{}' message='{}'", clock, p.audience, p.message);
            }
            //Packet::Unknown(_) => {
            _ => {
                //
            }
        }
    }

    let colors = [
        //BLACK,
        BLUE,
        CYAN,
        GREEN,
        MAGENTA,
        RED,
        WHITE,
        YELLOW,
    ];
    println!("Have {} tracks", points.len());
    for (i,(k,v)) in points.iter().enumerate() {
        //println!("{}", v.len());
        scatter_ctx.draw_series(
            v.iter()
                .map(|(x, y)| Circle::new((*x, *y), 1, colors[i % colors.len()].filled())),
        ).unwrap();
    }

    // Compute a histogram of packets
    /*let mut packet_counts = HashMap::new();
    for packet in packets.iter() {
        if !packet_counts.contains_key(&packet.packet_type) {
            packet_counts.insert(packet.packet_type, 0);
        }
        *packet_counts.get_mut(&packet.packet_type).unwrap() += 1;
    }
    let mut packet_counts: Vec<(_, _)> = packet_counts.iter().collect();
    packet_counts.sort();
    for (k,v) in packet_counts.iter() {
        println!("0x{:x}: {} instances", k, v);
    }
    println!("Found {} different packet types", packet_counts.len());*/

    // Some debugging code
    /*for (i,packet) in packets.iter().enumerate() {
        if packet.packet_type == 0x8 {
            println!("Dumping packet #{}: type 0x{:x} (len={}) at time {}:", i, packet.packet_type, packet.raw.len(), packet.clock);
            hexdump::hexdump(packet.raw);
        }
    }*/


    /*
    0x77 "info" packet:


    68 c1 da 5e a3 e0 07 00 00 4e
    80 02 7d
    71 01 28 4b 00 5d
    71 02 28 4e 4e 7d
    71 03 28 55 04 69 6e 66 6f
    71 04 7d 55 02 69 64
    71 05 4a bb e4 9b 3b 75 4e 4e 4e 4e 4e 4e 4e 4e 65 4b 01 5d
    71 06 28 4e 4e 7d
    71 07 28 68 04 7d 68 05 4b 00 75 4e 4e 4e 4e 4e 4e 4e 65 75 2e ff 6c 3e 00

    80 02 5d

    71 01 28 5d
    71 02 28 4b 00 4a 24 98 5f 3d 86
    71 03 4b 01 4a 9e 73 08 00 86
    71 04 4b 02 63 43 61 6d 6f 75 66 6c 61 67 65 49 6e 66 6f 0a 43 61 6d 6f 75 66 6c 61 67 65 49 6e 66 6f 0a
    71 05 4b 00 4b 00 86
    71 06 86
    71 07 4b 03 4a b3 b3 b3 00 86
    71 08 4b 04 4a 26 d2 9b 3b 86
    71 09 4b 05 58 05 00 00 00 57 2d 41 2d 57
    71 0a 86
    71 0b 4b 06 5d
    71 0c 28 49 34 32 39 33 30 34 33 39 32 30 0a 5d
    71 0d 28 4b 00 4b 00 4b 00 4b 00 65 65 86
    71 0e 4b 07 5d
    71 0f 28 49 34 32 36 30 33 35 30 38 39 36 0a 49 34 32 32 37 38 31 32 32 37 32 0a 65 86
    71 10 4b 08 4b 00 86
    71 11 4b 09 88 86
    71 12 4b 0a 4a 02 54 00 10 86
    71 13 4b 0b 4b 01 86
    71 14 4b 0c 89 86
    71 15 4b 0d 88 86
    71 16 4b 0e 89 86
    71 17 4b 0f 89 86
    71 18 4b 10 89 86
    71 19 4b 11 89 86 // 4b is a "suboject" specifier?
    71 1a 4b 12 89 86
    71 1b 4b 13 89 86
    71 1c 4b 14 4b 00 86
    71 1d 4b 15 4a fc 02 01 00 86
    71 1e 4b 16 55 0a 67 62 6c 61 63 6b 32 30 30 31
    71 1f 86
180 71 20 4b 17 63 50 6c 61 79 65 72 4d 6f 64 65 44 65 66 0a 50 6c 61 79 65 72 4d 6f 64 65 0a
    71 21 29 81
    71 22 7d
    71 23 55 09 66 69 78 65 64 44 69 63 74
    71 24 7d
    71 25 28 55 0e 70 6c 61 79 65 72 4d 6f 64 65 54 79 70 65
    71 26 4b 00 55 0e 6f 62 73 65 72 76 65 64 54 65 61 6d 49 64
    71 27 4b 00 75 73 62 86
    71 28 4b 18 4b 00 86
    71 29 4b 19 4b 00 86
    71 2a 4b 1a 4b 00 86
    71 2b 4b 1b 55 02 4e 41
    71 2c 86
    71 2d 4b 1c 7d
    71 2e 28 55 06 65 6e 67 69 6e 65
    71 2f 55 0a 41 42 31 5f 45 6e 67 69 6e 65
    71 30 55 0a 61 69 72 44 65 66 65 6e 73 65
    71 31 55 0c 42 5f 41 69 72 44 65 66 65 6e 73 65
    71 32 55 06 72 61 64 61 72 73
    71 33 55 09 41 42 5f 52 61 64 61 72 73
    71 34 55 09 61 72 74 69 6c 6c 65 72 79
    71 35 55 0a 41 42 31 5f 34 31 30 5f 34 35
    71 36 55 09 74 6f 72 70 65 64 6f 65 73
    71 37 55 10 54 6f 72 70 65 64 6f 65 73 44 65 66 61 75 6c 74
    71 38 55 04 61 74 62 61
    71 39 55 07 41 42 5f 41 54 42 41
    71 3a 55 05 73 63 6f 75 74
    71 3b 55 10 53 63 6f 75 74 54 79 70 65 44 65 66 61 75 6c 74
    71 3c 55 08 61 69 50 61 72 61 6d 73
    71 3d 55 08 41 49 50 61 72 61 6d 73
    71 3e 55 07 66 69 67 68 74 65 72
    71 3f 55 12 46 69 67 68 74 65 72 54 79 70 65 44 65 66 61 75 6c 74
    71 40 55 07 66 69 6e 64 65 72 73
    71 41 55 0a 41 42 5f 46 69 6e 64 65 72 73
310 71 42 55 0a 68 79 64 72 6f


     */
    /*for packet in packets.iter() {
        match packet {
            Packet { clock, payload: PacketType::Type8(p), .. } => {
                if p.subtype == 0x77 {
                    parse_77(p.payload);
                }
            }
            _ => {}
        }
    }*/

    /*
    Chat packets:
Message from "[-PF-]Tomillor" to global:
Dumping packet #2874: type 0x8 (len=36) at time 20.885942:
|0df20800 76000000 18000000 17f20800| ....v........... 00000000
|0d626174 746c655f 636f6d6d 6f6e0467| .battle_common.g 00000010
|6c686600|                            lhf.             00000020
                                                       00000024

Message from "showmeyournoobies" to team:
Dumping packet #113283: type 0x8 (len=57) at time 584.4276:
|0df20800 76000000 2d000000 11f20800| ....v...-....... 00000000
|0b626174 746c655f 7465616d 1b746861| .battle_team.tha 00000010
|74207368 6f756c64 20676976 65207573| t should give us 00000020
|20616e20 65646765 00|                 an edge.        00000030
                                                       00000039
     */

    /*let mut offset = 0;
    loop {
        let payload_size = u32::from_le_bytes(contents[offset..offset+4].try_into().unwrap());
        let packet_size = payload_size + 12;
        let packet_type = u32::from_le_bytes(contents[offset+4..offset+8].try_into().unwrap());
        let clock = f32::from_le_bytes(contents[offset+8..offset+12].try_into().unwrap());
        //println!("Found {} (0x{:x}) byte packet type=0x{:x} clock={}", packet_size, packet_size, packet_type, clock);
        let (_, packet) = parse_packet(&contents[offset..]).unwrap();
        println!("Found packet {:?}", packet);
        offset += packet_size as usize;
    }*/

    // Compute a histogram
    /*let mut hist = [0; 256];
    for b in remaining.iter() {
        hist[*b as usize] += 1;
    }
    for i in hist.iter() {
        println!("{}", i);
}*/
    //println!("{}", result.compressed_stream.len());
    let x: &[u8] = &result.compressed_stream[0..8];
    //println!("{}", x.len());
    //u64::from_le_bytes(x.try_into().unwrap())
    /*(
        u64::from_le_bytes(x.try_into().unwrap()),
        format!("{:?}", result.meta.dateTime)
    )*/
    //(result.compressed_stream[0], result.compressed_stream[7])//result.meta.scenarioConfigId as u8)
}

fn main() {
    //parse_replay(&std::path::PathBuf::from("replays/20200605_183626_PASB008-Colorado-1945_13_OC_new_dawn.wowsreplay"));
    //parse_replay("replays/20200605_185913_PRSB106-Izmail_08_NE_passage.wowsreplay");
    //parse_replay(&std::path::PathBuf::from("replays/20200605_112630_PASC207-Helena_10_NE_big_race.wowsreplay"));

    //let mut v = vec!();
    let mut paths: Vec<_> = std::fs::read_dir("replays/").unwrap().map(|e| { e.unwrap() }).collect();
    paths.sort_by(|a, b| { a.path().cmp(&b.path()) });
    for entry in paths {
        let path = entry.path();
        if !path.is_dir() {
            println!("{:?}", path);
            parse_replay(&path);
            //println!("{:?} -> 0x{:x}", path, r);
            //v.push((r,s));
        }
    }
}
