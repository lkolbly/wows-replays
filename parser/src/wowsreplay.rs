use crypto::symmetriccipher::BlockDecryptor;
use nom::bytes::complete::take;
use nom::multi::count;
use nom::number::complete::le_u32;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

use crate::error::*;

#[allow(non_snake_case)]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VehicleInfoMeta {
    pub shipId: u64,
    pub relation: u32,
    pub id: i64, // Account ID?
    pub name: String,
}

#[allow(non_snake_case)]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReplayMeta {
    pub matchGroup: String,
    pub gameMode: u32,
    pub gameType: String,
    pub clientVersionFromExe: String,
    pub scenarioUiCategoryId: u32,
    pub mapDisplayName: String,
    pub mapId: u32,
    pub clientVersionFromXml: String,
    pub weatherParams: HashMap<String, Vec<String>>,
    //mapBorder: Option<...>,
    pub duration: u32,
    pub gameLogic: Option<String>,
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
    pub logic: Option<String>,
    pub playerVehicle: String,
    pub battleDuration: u32,
}

#[derive(Debug)]
struct Replay<'a> {
    meta: ReplayMeta,
    raw_meta: &'a str,
    extra_data: Vec<&'a [u8]>,
    decompressed_size: u32,
    compressed_size: u32,
}

fn decode_meta(meta: &[u8]) -> Result<(&str, ReplayMeta), Error> {
    let raw_meta = std::str::from_utf8(meta)?;
    let meta: ReplayMeta = serde_json::from_str(raw_meta)?;
    Ok((raw_meta, meta))
}

fn parse_meta(i: &[u8]) -> IResult<&[u8], (&str, ReplayMeta)> {
    let (i, meta_len) = le_u32(i)?;
    let (i, raw_meta) = take(meta_len)(i)?;
    let meta = match decode_meta(raw_meta) {
        Ok(x) => x,
        Err(e) => {
            return Err(nom::Err::Error(e.into()));
        }
    };
    Ok((i, meta))
}

fn block(i: &[u8]) -> IResult<&[u8], &[u8]> {
    let (i, block_size) = le_u32(i)?;
    take(block_size)(i)
}

fn replay_format(i: &[u8]) -> IResult<&[u8], Replay> {
    let (i, magic) = le_u32(i)?;
    let (i, block_count) = le_u32(i)?;
    let (i, (raw_meta, meta)) = parse_meta(i)?;

    let (i, blocks) = count(block, (block_count as usize) - 1)(i)?;
    let (i, decompressed_size) = le_u32(i)?;
    let (i, compressed_size) = le_u32(i)?;
    Ok((
        i,
        Replay {
            meta: meta,
            raw_meta,
            extra_data: blocks,
            decompressed_size,
            compressed_size,
        },
    ))
}

#[derive(Debug)]
pub struct ReplayFile {
    pub meta: ReplayMeta,
    pub raw_meta: String,
    pub packet_data: Vec<u8>,
}

impl ReplayFile {
    pub fn from_decrypted_parts(meta: Vec<u8>, packet_data: Vec<u8>) -> Result<ReplayFile, Error> {
        let (_raw_meta, parsed_meta) = decode_meta(meta.as_slice())?;

        let raw_meta = String::from_utf8(meta)?;

        Ok(ReplayFile {
            meta: parsed_meta,
            raw_meta,
            packet_data,
        })
    }

    pub fn from_file(replay: &std::path::Path) -> Result<ReplayFile, ErrorKind> {
        let mut f = std::fs::File::open(replay).unwrap();
        let mut contents = vec![];
        f.read_to_end(&mut contents).unwrap();

        let (remaining, result) = replay_format(&contents)?;

        // Decrypt
        let key = [
            0x29, 0xB7, 0xC9, 0x09, 0x38, 0x3F, 0x84, 0x88, 0xFA, 0x98, 0xEC, 0x4E, 0x13, 0x19,
            0x79, 0xFB,
        ];
        let blowfish = crypto::blowfish::Blowfish::new(&key);
        assert!(blowfish.block_size() == 8);
        let encrypted = remaining; //result.compressed_stream
        let mut decrypted = vec![];
        decrypted.resize(encrypted.len(), 0u8);
        let num_blocks = encrypted.len() / blowfish.block_size();
        let mut previous = [0; 8]; // 8 == block size
        for i in 0..num_blocks {
            let offset = i * blowfish.block_size();
            blowfish.decrypt_block(
                &encrypted[offset..offset + blowfish.block_size()],
                &mut decrypted[offset..offset + blowfish.block_size()],
            );
            for j in 0..8 {
                decrypted[offset + j] = decrypted[offset + j] ^ previous[j];
                previous[j] = decrypted[offset + j];
            }
        }

        let mut deflater = flate2::read::ZlibDecoder::new(decrypted.as_slice());
        let mut contents = vec![];
        deflater.read_to_end(&mut contents).unwrap();

        Ok(ReplayFile {
            meta: result.meta,
            raw_meta: result.raw_meta.to_string(),
            packet_data: contents,
        })
    }
}
