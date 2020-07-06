use std::io::Read;
//use nom::{bytes::complete::take, bytes::complete::tag, named, do_parse, take, tag, number::complete::be_u16, number::complete::le_u16, number::complete::be_u8, alt, cond, number::complete::be_u24, char, opt, one_of, take_while, length_data, many1, complete, number::complete::le_u32, number::complete::le_f32, multi::many0, number::complete::be_u32, multi::count};
use nom::bytes::complete::take;
use nom::number::complete::le_u32;
use serde_derive::{Deserialize, Serialize};
//use thiserror::Error;
use std::collections::HashMap;
//use std::convert::TryInto;
use crypto::symmetriccipher::BlockDecryptor;
//use plotters::prelude::*;
//use image::{imageops::FilterType, ImageFormat};

use crate::error::*;

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct VehicleInfoMeta {
    pub shipId: u64,
    pub relation: u32,
    pub id: u64, // Account ID?
    pub name: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct ReplayMeta {
    pub matchGroup: String,
    pub gameMode: u32,
    pub clientVersionFromExe: String,
    pub scenarioUiCategoryId: u32,
    pub mapDisplayName: String,
    pub mapId: u32,
    pub clientVersionFromXml: String,
    pub weatherParams: HashMap<String, Vec<String>>,
    //mapBorder: Option<...>,
    pub duration: u32,
    pub gameLogic: String,
    pub name: String,
    pub scenario: String,
    pub playerID: u32,
    pub vehicles: Vec<VehicleInfoMeta>,
    pub playersPerTeam: u32,
    pub dateTime: String,
    pub mapName: String,
    pub playerName: String,
    pub scenarioConfigId: u32,
    pub teamsCount: u32,
    pub logic: String,
    pub playerVehicle: String,
    pub battleDuration: u32,
}

#[derive(Debug)]
struct Replay<'a> {
    meta: ReplayMeta,
    uncompressed_size: u32,
    compressed_stream: &'a [u8],
}

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
    let (i, _stream_size) = le_u32(i)?;
    Ok((i, Replay{
        meta: meta,
        uncompressed_size: uncompressed_size,
        compressed_stream: unknown,
    }))
}

#[derive(Debug)]
pub struct ReplayFile {
    pub meta: ReplayMeta,
    pub packet_data: Vec<u8>,
}

impl ReplayFile {
    pub fn from_file(replay: &std::path::PathBuf) -> ReplayFile {
        let mut f = std::fs::File::open(replay).unwrap();
        let mut contents = vec!();
        f.read_to_end(&mut contents).unwrap();

        //println!("{:x}", contents[0]);

        let (remaining, result) = replay_format(&contents).unwrap();
        //let meta_str = std::str::from_utf8(result.meta).unwrap();
        //println!("{:?}", meta_str);

        //let mut f = std::fs::File::create("meta.json").unwrap();
        //f.write_all(serde_json::to_string(&result.meta).unwrap().as_bytes()).unwrap();

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

        //println!("---------------------------------------------------------------");

        let mut deflater = flate2::read::ZlibDecoder::new(&decrypted[..]);
        let mut contents = vec!();
        deflater.read_to_end(&mut contents).unwrap();

        ReplayFile{
            meta: result.meta,
            packet_data: contents,
        }
    }
}
