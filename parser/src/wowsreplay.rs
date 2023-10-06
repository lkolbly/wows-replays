use crypto::symmetriccipher::BlockDecryptor;
use nom::bytes::complete::take;
use nom::number::complete::le_u32;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

use crate::error::*;

const REPLAY_SIGNATURE: [u8; 4] = [0x12, 0x32, 0x34, 0x11];

const BLOWFISH_BLOCK_SIZE: usize = 8; // the size of long
const BLOWFISH_KEY: [u8; 16] = [
    0x29, 0xB7, 0xC9, 0x09, 0x38, 0x3F, 0x84, 0x88, 
    0xFA, 0x98, 0xEC, 0x4E, 0x13, 0x19, 0x79, 0xFB,
];

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
    pub clientVersionFromExe: String,
    pub scenarioUiCategoryId: u32,
    pub mapDisplayName: String,
    pub mapId: u32,
    pub clientVersionFromXml: String,
    pub weatherParams: HashMap<String, Vec<String>>,
    //mapBorder: Option<...>,
    pub duration: u32,
    //pub gameLogic: String,
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
    //pub logic: String,
    pub playerVehicle: String,
    pub battleDuration: u32,
}

#[derive(Debug)]
struct Replay {
    meta: ReplayMeta,
    extra_data: Vec<String>,
}

fn decode_meta(meta: &[u8]) -> Result<ReplayMeta, Error> {
    let meta = std::str::from_utf8(meta)?;
    let meta: ReplayMeta = serde_json::from_str(meta)?;
    Ok(meta)
}

fn parse_meta(i: &[u8]) -> IResult<&[u8], ReplayMeta> {
    let (i, meta_len) = le_u32(i)?;
    let (i, meta) = take(meta_len)(i)?;
    let meta = match decode_meta(meta) {
        Ok(x) => x,
        Err(e) => {
            return Err(nom::Err::Error(e.into()));
        }
    };
    Ok((i, meta))
}

fn replay_format(i: &[u8]) -> IResult<&[u8], Replay> {
    let (i, signature) = take(4usize)(i)?;
    if signature != REPLAY_SIGNATURE {
        return Err(failure_from_kind(ErrorKind::InvalidReplaySignature));
    }

    let (i, blocks_count) = le_u32(i)?;
    let (i, meta) = parse_meta(i)?;
    // 12.6.0 adds extra data
    let (i, extra_data) = extra_format(i, blocks_count)?;
    Ok((
        i,
        Replay {
            meta,
            extra_data
        },
    ))
}

/// Extra data block added in 12.6.0
fn extra_format(i: &[u8], blocks_count: u32) -> IResult<&[u8], Vec<String>> {
    let mut json_list = Vec::new();
    let mut p = i;
    for _ in 0..blocks_count - 1 {
        // read 4 bytes for the block size
        let (i, block_size) = le_u32(p)?;
        let (i, block) = take(block_size)(i)?;
        p = i; // update the pointer

        // try to read this as a json string
        let block = std::str::from_utf8(block);
        if block.is_err() {
            continue;
        }

        let block = block.unwrap();
        json_list.push(block.to_string());
    }

    Ok((p, json_list))
}

#[derive(Debug)]
pub struct ReplayFile {
    pub meta: ReplayMeta,
    pub packet_data: Vec<u8>,
    pub extra_data: Vec<String>,
}

impl ReplayFile {
    pub fn from_file(replay: &std::path::PathBuf) -> Result<ReplayFile, ErrorKind> {
        let mut f = std::fs::File::open(replay).unwrap();
        let mut contents = vec![];
        f.read_to_end(&mut contents).unwrap();

        let (remaining, result) = replay_format(&contents)?;

        // Decrypt
        let blowfish = crypto::blowfish::Blowfish::new(&BLOWFISH_KEY);
        if blowfish.block_size() != BLOWFISH_BLOCK_SIZE {
            return Err(ErrorKind::InvalidBlowfishBlockSize);
        }
        let encrypted = remaining; //result.compressed_stream
        let mut decrypted = vec![];
        decrypted.resize(encrypted.len(), 0u8);
        let num_blocks = encrypted.len() / BLOWFISH_BLOCK_SIZE;
        let mut previous: [u8; 8] = [0; BLOWFISH_BLOCK_SIZE];
        let mut has_previous = false;
        for i in 0..num_blocks {
            // first block is not used
            if i == 0 {
                continue;
            }

            let offset = i * BLOWFISH_BLOCK_SIZE;
            blowfish.decrypt_block(
                &encrypted[offset..offset + BLOWFISH_BLOCK_SIZE],
                &mut decrypted[offset..offset + BLOWFISH_BLOCK_SIZE],
            );

            for j in 0..BLOWFISH_BLOCK_SIZE {
                if has_previous {
                    decrypted[offset + j] ^= previous[j];
                }
                previous[j] = decrypted[offset + j];
                has_previous = true;
            }
        }

        // validate the decrypted data's Zlib header, ignore first chunk because it was skipped
        let decrypted = &decrypted[8..];
        if decrypted[0] != 0x78 {
            return Err(ErrorKind::InvalidZlibHeader);
        }
        let mut deflater = flate2::read::ZlibDecoder::new(decrypted);
        let mut contents = vec![];
        deflater.read_to_end(&mut contents).unwrap();

        Ok(ReplayFile {
            meta: result.meta,
            packet_data: contents,
            extra_data: result.extra_data,
        })
    }
}
