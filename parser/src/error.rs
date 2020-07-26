//use std::io::Write;
//use nom::{bytes::complete::take, bytes::complete::tag, named, do_parse, take, tag, number::complete::be_u16, number::complete::le_u16, number::complete::be_u8, alt, cond, number::complete::be_u24, char, opt, one_of, take_while, length_data, many1, complete, number::complete::le_u32, number::complete::le_f32, multi::many0, number::complete::be_u32, multi::count};
//use serde_derive::{Deserialize, Serialize};
use thiserror::Error;
//use std::collections::HashMap;
//use std::convert::TryInto;
//use crypto::symmetriccipher::BlockDecryptor;
//use plotters::prelude::*;
//use image::{imageops::FilterType, ImageFormat};

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    backtrace: Vec<ErrorKind>,
}

#[derive(Error, Debug)]
pub enum ErrorKind {
    #[error("Nom error")]
    Nom {
        err: nom::error::ErrorKind,
        input: Vec<u8>,
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
    #[error("Unsupported replay file version found")]
    UnsupportedReplayVersion(u32),
    #[error("Unable to process packet")]
    UnableToProcessPacket{
        supertype: u32,
        subtype: u32,
        reason: String,
        packet: Vec<u8>,
    },
}

impl nom::error::ParseError<&[u8]> for Error {
    fn from_error_kind(input: &[u8], kind: nom::error::ErrorKind) -> Self {
        Self {
            kind: ErrorKind::Nom { err: kind, input: input.to_vec() },
            backtrace: Vec::new()
        }
    }

    fn append(input: &[u8], kind: nom::error::ErrorKind, mut other: Self) -> Self {
        other.backtrace.push(Self::from_error_kind(input, kind).kind);
        other
    }
}

impl std::convert::From<nom::Err<Error>> for ErrorKind {
    fn from(x: nom::Err<Error>) -> ErrorKind {
        match x {
            nom::Err::<Error>::Incomplete(_) => { panic!("We can't handle incomplete replay files"); },
            nom::Err::<Error>::Error(e) => { e.kind },
            nom::Err::<Error>::Failure(e) => { e.kind },
        }
    }
}

impl std::convert::From<std::str::Utf8Error> for Error {
    fn from(x: std::str::Utf8Error) -> Error {
        Error {
            kind: x.into(),
            backtrace: vec!(),
        }
    }
}

impl std::convert::From<serde_json::Error> for Error {
    fn from(x: serde_json::Error) -> Error {
        Error {
            kind: x.into(),
            backtrace: vec!(),
        }
    }
}

pub type IResult<I, T> = nom::IResult<I, T, Error>;

pub fn failure_from_kind(kind: ErrorKind) -> nom::Err<Error> {
    nom::Err::Failure(Error {
        kind: kind.into(),
        backtrace: vec!(),
    })
}
