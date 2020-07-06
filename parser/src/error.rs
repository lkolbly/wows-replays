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
pub struct Error<I: std::fmt::Debug> {
    pub kind: ErrorKind<I>,
    backtrace: Vec<ErrorKind<I>>,
}

#[derive(Error, Debug)]
pub enum ErrorKind<I: std::fmt::Debug> {
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

pub type IResult<I, T> = nom::IResult<I, T, Error<I>>;
