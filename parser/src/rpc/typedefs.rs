use crate::error::*;
use nom::number::complete::{le_i16, le_i32, le_i64, le_i8, le_u64, le_u8};
use nom::{
    bytes::complete::take, number::complete::le_f32, number::complete::le_f64,
    number::complete::le_u16, number::complete::le_u32,
};
use serde::ser::{SerializeMap, SerializeSeq, SerializeTuple};
use std::collections::HashMap;
use std::convert::TryInto;

pub type TypeAliases = HashMap<String, ArgType>;

fn child_by_name<'a, 'b>(
    node: &roxmltree::Node<'a, 'b>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'b>> {
    for child in node.children() {
        if child.tag_name().name() == name {
            return Some(child);
        }
    }
    None
}

#[derive(Clone, Debug, PartialEq)]
pub enum PrimitiveType {
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Int8,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
    Vector2,
    Vector3,
    String,
    UnicodeString,
    Blob,
}

impl PrimitiveType {
    fn parse_value<'replay, 'argtype>(
        &'argtype self,
        i: &'replay [u8],
    ) -> IResult<&'replay [u8], ArgValue<'argtype>> {
        match self {
            PrimitiveType::Uint8 => {
                let (i, v) = le_u8(i)?;
                Ok((i, ArgValue::Uint8(v)))
            }
            PrimitiveType::Uint16 => {
                let (i, v) = le_u16(i)?;
                Ok((i, ArgValue::Uint16(v)))
            }
            PrimitiveType::Uint32 => {
                let (i, v) = le_u32(i)?;
                Ok((i, ArgValue::Uint32(v)))
            }
            PrimitiveType::Uint64 => {
                let (i, v) = le_u64(i)?;
                Ok((i, ArgValue::Uint64(v)))
            }
            PrimitiveType::Int8 => {
                let (i, v) = le_i8(i)?;
                Ok((i, ArgValue::Int8(v)))
            }
            PrimitiveType::Int16 => {
                let (i, v) = le_i16(i)?;
                Ok((i, ArgValue::Int16(v)))
            }
            PrimitiveType::Int32 => {
                let (i, v) = le_i32(i)?;
                Ok((i, ArgValue::Int32(v)))
            }
            PrimitiveType::Int64 => {
                let (i, v) = le_i64(i)?;
                Ok((i, ArgValue::Int64(v)))
            }
            PrimitiveType::Float32 => {
                let (i, v) = le_f32(i)?;
                Ok((i, ArgValue::Float32(v)))
            }
            PrimitiveType::Float64 => {
                let (i, v) = le_f64(i)?;
                Ok((i, ArgValue::Float64(v)))
            }
            PrimitiveType::Vector2 => {
                let (i, x) = le_f32(i)?;
                let (i, y) = le_f32(i)?;
                Ok((i, ArgValue::Vector2((x, y))))
            }
            PrimitiveType::Vector3 => {
                let (i, x) = le_f32(i)?;
                let (i, y) = le_f32(i)?;
                let (i, z) = le_f32(i)?;
                Ok((i, ArgValue::Vector3((x, y, z))))
            }
            PrimitiveType::Blob => {
                let (i, size) = le_u8(i)?;
                if size == 0xff {
                    let (i, size) = le_u16(i)?;
                    let (i, _unknown) = le_u8(i)?;
                    let (i, data) = take(size)(i)?;
                    Ok((i, ArgValue::Blob(data.to_vec())))
                } else {
                    let (i, data) = take(size)(i)?;
                    Ok((i, ArgValue::Blob(data.to_vec())))
                }
            }
            PrimitiveType::String => {
                let (i, size) = le_u8(i)?;
                if size == 0xff {
                    let (i, size) = le_u16(i)?;
                    let (i, _unknown) = le_u8(i)?;
                    let (i, data) = take(size)(i)?;
                    Ok((i, ArgValue::String(data.to_vec())))
                } else {
                    let (i, data) = take(size)(i)?;
                    Ok((i, ArgValue::String(data.to_vec())))
                }
            }
            PrimitiveType::UnicodeString => {
                let (i, size) = le_u8(i)?;
                if size == 0xff {
                    let (i, size) = le_u16(i)?;
                    let (i, _unknown) = le_u8(i)?;
                    let (i, data) = take(size)(i)?;
                    Ok((i, ArgValue::UnicodeString(data.to_vec())))
                } else {
                    let (i, data) = take(size)(i)?;
                    Ok((i, ArgValue::UnicodeString(data.to_vec())))
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FixedDictProperty {
    pub name: String,
    pub prop_type: ArgType,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ArgType {
    Primitive(PrimitiveType),
    Array((Option<usize>, Box<ArgType>)),

    /// (allow_none, properties)
    FixedDict((bool, Vec<FixedDictProperty>)),
    Tuple((Box<ArgType>, usize)),
}

#[derive(Clone, Debug, PartialEq, variantly::Variantly)]
pub enum ArgValue<'argtype> {
    Uint8(u8),
    Uint16(u16),
    Uint32(u32),
    Uint64(u64),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Float32(f32),
    Float64(f64),
    Vector2((f32, f32)),
    Vector3((f32, f32, f32)),
    String(Vec<u8>),
    UnicodeString(Vec<u8>),
    Blob(Vec<u8>),
    Array(Vec<ArgValue<'argtype>>),
    FixedDict(HashMap<&'argtype str, ArgValue<'argtype>>),
    NullableFixedDict(Option<HashMap<&'argtype str, ArgValue<'argtype>>>),
    Tuple(Vec<ArgValue<'argtype>>),
}

impl<'argtype> serde::Serialize for ArgValue<'argtype> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        //serializer.serialize_i32(5)
        match self {
            Self::Uint8(i) => serializer.serialize_u8(*i),
            Self::Uint16(i) => serializer.serialize_u16(*i),
            Self::Uint32(i) => serializer.serialize_u32(*i),
            Self::Uint64(i) => serializer.serialize_u64(*i),
            Self::Int8(i) => serializer.serialize_i8(*i),
            Self::Int16(i) => serializer.serialize_i16(*i),
            Self::Int32(i) => serializer.serialize_i32(*i),
            Self::Int64(i) => serializer.serialize_i64(*i),
            Self::Float32(f) => serializer.serialize_f32(*f),
            Self::Float64(f) => serializer.serialize_f64(*f),
            Self::Vector2((x, y)) => {
                let mut tup = serializer.serialize_tuple(2)?;
                tup.serialize_element(x)?;
                tup.serialize_element(y)?;
                tup.end()
            }
            Self::Vector3((x, y, z)) => {
                let mut tup = serializer.serialize_tuple(3)?;
                tup.serialize_element(x)?;
                tup.serialize_element(y)?;
                tup.serialize_element(z)?;
                tup.end()
            }
            Self::String(s) => serializer.serialize_bytes(&s),
            Self::UnicodeString(s) => serializer.serialize_bytes(&s),
            Self::Blob(blob) => {
                // TODO: Determine when we can/can't pickle-decode this
                // Also, make pickled::Value implement Serialize
                let decoded: Result<serde_json::Value, _> =
                    pickled::from_slice(blob, pickled::de::DeOptions::new());
                match decoded {
                    Ok(v) => {
                        //let x: serde_json::Value = v.try_into().unwrap();
                        serializer.serialize_some(&v)
                    }
                    Err(_) => serializer.serialize_bytes(&blob),
                }
            }
            Self::Array(a) => {
                let mut seq = serializer.serialize_seq(Some(a.len()))?;
                for element in a.iter() {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
            Self::FixedDict(d) => {
                let mut obj = serializer.serialize_map(Some(d.len()))?;
                for (k, v) in d.iter() {
                    obj.serialize_entry(k, v)?;
                }
                obj.end()
            }
            Self::NullableFixedDict(Some(d)) => {
                let mut obj = serializer.serialize_map(Some(d.len()))?;
                for (k, v) in d.iter() {
                    obj.serialize_entry(k, v)?;
                }
                obj.end()
            }
            Self::NullableFixedDict(None) => serializer.serialize_none(),
            Self::Tuple(_t) => {
                unimplemented!();
            }
        }
    }
}

const INFINITY: usize = 0xffff;

impl ArgType {
    pub fn sort_size(&self) -> usize {
        match self {
            Self::Primitive(PrimitiveType::Uint8) => 1,
            Self::Primitive(PrimitiveType::Uint16) => 2,
            Self::Primitive(PrimitiveType::Uint32) => 4,
            Self::Primitive(PrimitiveType::Uint64) => 8,
            Self::Primitive(PrimitiveType::Int8) => 1,
            Self::Primitive(PrimitiveType::Int16) => 2,
            Self::Primitive(PrimitiveType::Int32) => 4,
            Self::Primitive(PrimitiveType::Int64) => 8,
            Self::Primitive(PrimitiveType::Float32) => 4,
            Self::Primitive(PrimitiveType::Float64) => 8,
            Self::Primitive(PrimitiveType::Vector2) => 8,
            Self::Primitive(PrimitiveType::Vector3) => 12,
            Self::Primitive(PrimitiveType::String) => INFINITY,
            Self::Primitive(PrimitiveType::UnicodeString) => INFINITY,
            Self::Primitive(PrimitiveType::Blob) => INFINITY,
            Self::Array((None, _)) => INFINITY,
            Self::Array((Some(count), t)) => {
                let sort_size = t.sort_size();
                if sort_size == INFINITY {
                    INFINITY
                } else {
                    sort_size * count
                }
            }
            Self::FixedDict((allow_none, props)) => {
                if *allow_none {
                    return INFINITY;
                }
                props
                    .iter()
                    .map(|x| x.prop_type.sort_size())
                    .fold(0, |a, b| {
                        if a == INFINITY || b == INFINITY {
                            INFINITY
                        } else {
                            a + b
                        }
                    })
            }
            Self::Tuple((t, count)) => {
                let sort_size = t.sort_size();
                if sort_size == INFINITY {
                    INFINITY
                } else {
                    sort_size * count
                }
            }
        }
    }

    pub fn parse_value<'a, 'b>(&'b self, i: &'a [u8]) -> IResult<&'a [u8], ArgValue<'b>> {
        match self {
            Self::Primitive(p) => p.parse_value(i),
            Self::Array((count, atype)) => {
                let mut values = vec![];
                let (mut i, length) = match count {
                    Some(count) => (i, *count as u8),
                    None => le_u8(i)?,
                };
                for _ in 0..length {
                    let (new_i, element) = atype.parse_value(i)?;
                    i = new_i;
                    values.push(element);
                }
                Ok((i, ArgValue::Array(values)))
            }
            Self::FixedDict((allow_none, props)) => {
                let mut dict: HashMap<&'b str, ArgValue<'b>> = HashMap::new();
                let mut i = i;
                //println!();
                //println!("{} {:?}", allow_none, i);
                if *allow_none {
                    let (new_i, flag) = le_u8(i)?;
                    i = new_i;
                    if flag == 0 {
                        return Ok((i, ArgValue::NullableFixedDict(None)));
                    } else if flag != 1 {
                        return Err(failure_from_kind(ErrorKind::UnknownFixedDictFlag {
                            flag,
                            packet: i.to_vec(),
                        }));
                        //panic!("Unknown fixed dict flag {:?} in {:?}", flag, i);
                    }
                }
                for property in props.iter() {
                    //println!("{:?} {:?}", property.prop_type, i);
                    let (new_i, element) = property.prop_type.parse_value(i)?;
                    i = new_i;
                    dict.insert(&property.name, element);
                }
                if *allow_none {
                    Ok((i, ArgValue::NullableFixedDict(Some(dict))))
                } else {
                    Ok((i, ArgValue::FixedDict(dict)))
                }
            }
            Self::Tuple((_t, _count)) => {
                panic!("Tuple parsing is unsupported");
            }
        }
    }
}

pub fn parse_type(arg: &roxmltree::Node, aliases: &HashMap<String, ArgType>) -> ArgType {
    let t = arg.first_child().unwrap().text().unwrap().trim();
    if t == "UINT8" {
        ArgType::Primitive(PrimitiveType::Uint8)
    } else if t == "UINT16" {
        ArgType::Primitive(PrimitiveType::Uint16)
    } else if t == "UINT32" {
        ArgType::Primitive(PrimitiveType::Uint32)
    } else if t == "UINT64" {
        ArgType::Primitive(PrimitiveType::Uint64)
    } else if t == "INT8" {
        ArgType::Primitive(PrimitiveType::Int8)
    } else if t == "INT16" {
        ArgType::Primitive(PrimitiveType::Int16)
    } else if t == "INT32" {
        ArgType::Primitive(PrimitiveType::Int32)
    } else if t == "INT64" {
        ArgType::Primitive(PrimitiveType::Int64)
    } else if t == "FLOAT32" {
        ArgType::Primitive(PrimitiveType::Float32)
    } else if t == "FLOAT" {
        // Note that "FLOAT64" is Float64
        ArgType::Primitive(PrimitiveType::Float32)
    } else if t == "STRING" {
        ArgType::Primitive(PrimitiveType::String)
    } else if t == "UNICODE_STRING" {
        ArgType::Primitive(PrimitiveType::UnicodeString)
    } else if t == "VECTOR2" {
        ArgType::Primitive(PrimitiveType::Vector2)
    } else if t == "VECTOR3" {
        ArgType::Primitive(PrimitiveType::Vector3)
    } else if t == "BLOB" {
        ArgType::Primitive(PrimitiveType::Blob)
    } else if t == "USER_TYPE" || t == "MAILBOX" || t == "PYTHON" {
        // TODO: This is a HACKY HACKY workaround for things we don't recognize
        ArgType::Primitive(PrimitiveType::Blob)
    } else if t == "ARRAY" {
        let subtype = parse_type(&child_by_name(arg, "of").unwrap(), aliases);
        /*let subtype = match subtype {
            ArgType::Primitive(p) => p,
            _ => {
                panic!("Unsupported array subtype {:?}", subtype);
            }
        };*/
        let count = child_by_name(arg, "size")
            .map(|count| count.text().unwrap().trim().parse::<usize>().unwrap());
        ArgType::Array((count, Box::new(subtype)))
    } else if t == "FIXED_DICT" {
        let mut props = vec![];
        //println!("{:#?}", arg);
        let allow_none = match child_by_name(&arg, "AllowNone") {
            Some(_) => true, // TODO: Check if the text is actually "true"
            None => false,
        };
        let properties = match child_by_name(&arg, "Properties") {
            Some(p) => p,
            None => {
                return ArgType::FixedDict((allow_none, vec![]));
            }
        };
        for prop in properties.children() {
            if !prop.is_element() {
                continue;
            }
            let name = prop.tag_name().name();
            let prop_type = child_by_name(&prop, "Type").unwrap();
            let prop_type = parse_type(&prop_type, aliases);
            props.push(FixedDictProperty {
                name: name.to_string(),
                prop_type,
            });
        }
        ArgType::FixedDict((allow_none, props))
    } else if t == "TUPLE" {
        let subtype = parse_type(&child_by_name(arg, "of").unwrap(), aliases);
        let count = child_by_name(arg, "size")
            .unwrap()
            .text()
            .unwrap()
            .trim()
            .parse::<usize>()
            .unwrap();
        ArgType::Tuple((Box::new(subtype), count))
    } else if aliases.contains_key(t) {
        aliases.get(t).unwrap().clone()
    } else {
        panic!("Unrecognized type {}", t);
    }
}

pub fn parse_aliases(def: &[u8]) -> HashMap<String, ArgType> {
    let def = std::str::from_utf8(def).unwrap();
    let mut aliases = HashMap::new();

    //let def = std::fs::read_to_string(&file).unwrap();
    let doc = roxmltree::Document::parse(def).unwrap();
    let root = doc.root();

    for t in root.first_child().unwrap().children() {
        if !t.is_element() {
            continue;
        }
        //println!("{}", t.tag_name().name());
        aliases.insert(t.tag_name().name().to_string(), parse_type(&t, &aliases));
    }
    //println!("Found {} type aliases", aliases.len());
    aliases
}

macro_rules! into_unwrappable_type {
    ($t: ty, $tag: path) => {
        impl<'a> std::convert::TryInto<$t> for &ArgValue<'a> {
            type Error = ();

            fn try_into(self) -> Result<$t, Self::Error> {
                match self {
                    $tag(i) => Ok(*i),
                    _ => Err(()),
                }
            }
        }
    };
}

into_unwrappable_type!(u8, ArgValue::Uint8);
into_unwrappable_type!(u16, ArgValue::Uint16);
into_unwrappable_type!(u32, ArgValue::Uint32);
into_unwrappable_type!(u64, ArgValue::Uint64);
into_unwrappable_type!(i8, ArgValue::Int8);
into_unwrappable_type!(i16, ArgValue::Int16);
into_unwrappable_type!(i32, ArgValue::Int32);
into_unwrappable_type!(i64, ArgValue::Int64);
into_unwrappable_type!(f32, ArgValue::Float32);
into_unwrappable_type!(f64, ArgValue::Float64);

impl<'a, 'b, T> std::convert::TryFrom<&'b ArgValue<'a>> for Vec<T>
where
    &'b ArgValue<'a>: std::convert::TryInto<T, Error = ()>,
{
    type Error = ();

    fn try_from(value: &'b ArgValue<'a>) -> Result<Self, Self::Error> {
        match value {
            ArgValue::Array(v) => {
                let result: Result<Vec<T>, Self::Error> = v.iter().map(|x| x.try_into()).collect();
                result
            }
            _ => Err(()),
        }
    }
}

#[macro_export]
macro_rules! unpack_rpc_args {
    ($args: ident, $($t: ty),+) => {
        {
            let mut i = 0;
            ($({
                let x: $t = <&crate::rpc::typedefs::ArgValue as std::convert::TryInto<$t>>::try_into(&$args[i]).unwrap();
                i += 1;
                let _ = i; // Ignore "assigned variable never read" error
                x
            }),+,)
        }
    };
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_argtype() {
        let doc = "<Arg> UINT8 </Arg>";
        let doc = roxmltree::Document::parse(&doc).unwrap();
        let root = doc.root();
        assert_eq!(
            parse_type(&root, &HashMap::new()),
            ArgType::Primitive(PrimitiveType::Uint8)
        );
    }

    #[test]
    fn test_int16() {
        let doc = "<Arg> INT16 </Arg>";
        let doc = roxmltree::Document::parse(&doc).unwrap();
        let root = doc.root();
        assert_eq!(
            parse_type(&root, &HashMap::new()),
            ArgType::Primitive(PrimitiveType::Int16)
        );
    }

    #[test]
    fn test_fixed_dict() {
        let doc = "<Arg>
            FIXED_DICT
            <Properties>
                <byShip><Type>FLOAT</Type></byShip>
                <byPlane><Type>FLOAT</Type></byPlane>
                <bySmoke><Type>FLOAT</Type></bySmoke>
            </Properties>
        </Arg>";
        let doc = roxmltree::Document::parse(&doc).unwrap();
        let root = doc.root_element();
        let t = parse_type(&root, &HashMap::new());
        assert_eq!(
            t,
            ArgType::FixedDict((
                false,
                vec![
                    FixedDictProperty {
                        name: "byShip".to_string(),
                        prop_type: ArgType::Primitive(PrimitiveType::Float32),
                    },
                    FixedDictProperty {
                        name: "byPlane".to_string(),
                        prop_type: ArgType::Primitive(PrimitiveType::Float32),
                    },
                    FixedDictProperty {
                        name: "bySmoke".to_string(),
                        prop_type: ArgType::Primitive(PrimitiveType::Float32),
                    }
                ]
            ))
        );
        assert_eq!(t.sort_size(), 12);
    }

    #[test]
    fn test_crew_modifiers() {
        let alias = "<CREW_MODIFIERS_COMPACT_PARAMS>
            FIXED_DICT
            <Properties>
                <paramsId><Type>UINT32</Type></paramsId>
                <isInAdaptation><Type>BOOL</Type></isInAdaptation>
                <learnedSkills><Type>ARRAY<of>ARRAY<of>UINT8</of></of></Type></learnedSkills>
            </Properties>
            <implementedBy>CrewModifiers.crewModifiersCompactParamsConverter</implementedBy>
        </CREW_MODIFIERS_COMPACT_PARAMS>";
        let doc = roxmltree::Document::parse(&alias).unwrap();
        let root = doc.root_element();
        let mut aliases = HashMap::new();
        aliases.insert("BOOL".to_string(), ArgType::Primitive(PrimitiveType::Uint8));
        aliases.insert(
            "CREW_MODIFIERS_COMPACT_PARAMS".to_string(),
            parse_type(&root, &aliases),
        );

        let proptype = "<Type>CREW_MODIFIERS_COMPACT_PARAMS</Type>";
        let doc = roxmltree::Document::parse(&proptype).unwrap();
        let root = doc.root();
        let t = parse_type(&root, &aliases);
        assert_eq!(t.sort_size(), 65535);
    }

    #[test]
    fn test_fixeddict_allownone() {
        let spec = "<TRIGGERS_STATE>
            FIXED_DICT
            <Properties>
                <modifier><Type> MODIFIER_STATE </Type></modifier>
            </Properties>
            <AllowNone>true</AllowNone>
        </TRIGGERS_STATE>";
        let mut aliases = HashMap::new();
        aliases.insert(
            "MODIFIER_STATE".to_string(),
            ArgType::Primitive(PrimitiveType::Uint32),
        );

        let doc = roxmltree::Document::parse(&spec).unwrap();
        let root = doc.root_element();
        let t = parse_type(&root, &aliases);
        //println!("{:#?}", t);

        let data = [0];
        let (i, data) = t.parse_value(&data).unwrap();
        assert_eq!(i.len(), 0);
        assert_eq!(data, ArgValue::NullableFixedDict(None));

        let data = [1, 5, 0, 0, 0];
        let (i, data) = t.parse_value(&data).unwrap();
        assert_eq!(i.len(), 0);
        let m = match data {
            ArgValue::NullableFixedDict(Some(h)) => h,
            _ => panic!(),
        };
        assert_eq!(*m.get("modifier").unwrap(), ArgValue::Uint32(5));
    }

    #[test]
    fn test_fixedsize_array() {
        let spec = "<Type>ARRAY<of>UINT16</of><size>2</size></Type>";
        let doc = roxmltree::Document::parse(&spec).unwrap();
        let root = doc.root_element();
        let mut aliases = HashMap::new();
        let t = parse_type(&root, &aliases);
        //println!("{:#?}", t);

        let data = [1, 0, 3, 0];
        let (i, data) = t.parse_value(&data).unwrap();
        assert_eq!(i.len(), 0);
        assert_eq!(
            data,
            ArgValue::Array(vec![ArgValue::Uint16(1), ArgValue::Uint16(3)])
        );
    }

    #[test]
    fn test_unpacker_macro_single() {
        let args = vec![ArgValue::Uint8(5)];
        let (u8_arg,) = unpack_rpc_args!(args, u8);
        assert_eq!(u8_arg, 5);
    }

    #[test]
    fn test_unpacker_macro() {
        let args = vec![
            ArgValue::Uint8(5),
            ArgValue::Int32(-54),
            ArgValue::Array(vec![ArgValue::Uint16(1), ArgValue::Uint16(3)]),
            //ArgValue::NullableFixedDict(None),
            //ArgValue::NullableFixedDict(Some(HashMap::new())),
            //ArgValue::String("Hello, world!".to_string()),
        ];
        let args = unpack_rpc_args!(args, u8, i32, Vec<u16>);
        assert_eq!(args.0, 5);
        assert_eq!(args.1, -54);
        assert_eq!(args.2, vec![1, 3]);
    }
}
