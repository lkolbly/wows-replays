use crate::rpc::typedefs::{ArgType, ArgValue};
use bitreader::BitReader;
use serde_derive::Serialize;

#[derive(Debug, Serialize)]
pub enum PropertyNestLevel<'argtype> {
    ArrayIndex(usize),
    DictKey(&'argtype str),
}

#[derive(Debug, Serialize)]
pub enum UpdateAction<'argtype> {
    SetKey {
        key: &'argtype str,
        value: ArgValue<'argtype>,
    },
    SetRange {
        start: usize,
        stop: usize,
        values: Vec<ArgValue<'argtype>>,
    },
    SetElement {
        index: usize,
        value: ArgValue<'argtype>,
    },
    RemoveElement {
        index: usize,
    },
    RemoveRange {
        start: usize,
        stop: usize,
    },
}

#[derive(Debug, Serialize)]
pub struct PropertyNesting<'argtype> {
    levels: Vec<PropertyNestLevel<'argtype>>,
    action: UpdateAction<'argtype>,
}

/// This function emulates Python's slice semantics
fn slice_insert<T>(idx1: usize, idx2: usize, target: &mut Vec<T>, mut source: Vec<T>) {
    // First we delete target[idx1..idx2]
    for _ in idx1..idx2 {
        if target.len() <= idx1 {
            break;
        }
        target.remove(idx1);
    }

    // Then we insert source[..] into target[idx1] repeatedly
    for (i, elem) in source.drain(..).enumerate() {
        target.insert(std::cmp::min(idx1 + i, target.len()), elem);
    }
}

fn nested_update_command<'argtype>(
    is_slice: bool,
    t: &'argtype ArgType,
    mut prop_value: &mut ArgValue<'argtype>,
    mut reader: BitReader,
) -> PropertyNesting<'argtype> {
    match (t, &mut prop_value) {
        (ArgType::FixedDict((_, entries)), _) => {
            let entry_idx = reader
                .read_u8(entries.len().next_power_of_two().trailing_zeros() as u8)
                .unwrap();
            while reader.remaining() % 8 != 0 {
                reader.read_u8(1).unwrap();
            }
            let mut remaining = vec![0; reader.remaining() as usize / 8];
            reader.read_u8_slice(&mut remaining[..]).unwrap();
            assert!(reader.remaining() == 0);
            let (_, value) = entries[entry_idx as usize]
                .prop_type
                .parse_value(&remaining[..])
                .unwrap();
            match prop_value {
                ArgValue::FixedDict(d) => {
                    d.insert(&entries[entry_idx as usize].name, value.clone());
                }
                ArgValue::NullableFixedDict(Some(d)) => {
                    d.insert(&entries[entry_idx as usize].name, value.clone());
                }
                ArgValue::NullableFixedDict(None) => unimplemented!(),
                _ => panic!("FixedDict type caused unexpected value {:?}", prop_value),
            }
            return PropertyNesting {
                levels: vec![],
                action: UpdateAction::SetKey {
                    key: &entries[entry_idx as usize].name,
                    value: value,
                },
            };
        }
        (ArgType::Array((_size, element_type)), ArgValue::Array(ref mut elements)) => {
            let idx_bits = if is_slice {
                elements.len() + 1
            } else {
                elements.len()
            }
            .next_power_of_two()
            .trailing_zeros();
            let idx1 = reader.read_u8(idx_bits as u8).unwrap();
            let idx2 = if is_slice {
                Some(reader.read_u8(idx_bits as u8).unwrap())
            } else {
                None
            };

            while reader.remaining() % 8 != 0 {
                reader.read_u8(1).unwrap();
            }
            let mut remaining = vec![0; reader.remaining() as usize / 8];
            reader.read_u8_slice(&mut remaining[..]).unwrap();

            if remaining.len() == 0 {
                // Remove elements
                if is_slice {
                    slice_insert(idx1 as usize, idx2.unwrap() as usize, elements, vec![]);
                    return PropertyNesting {
                        levels: vec![],
                        action: UpdateAction::RemoveRange {
                            start: idx1 as usize,
                            stop: idx2.unwrap() as usize,
                        },
                    };
                } else {
                    unimplemented!();
                }
            }

            let mut new_elements = vec![];
            let mut i = &remaining[..];
            while i.len() > 0 {
                let (new_i, element) = element_type.parse_value(i).unwrap();
                i = new_i;
                new_elements.push(element);
            }

            if is_slice {
                slice_insert(
                    idx1 as usize,
                    idx2.unwrap() as usize,
                    elements,
                    new_elements.clone(),
                );
                return PropertyNesting {
                    levels: vec![],
                    action: UpdateAction::SetRange {
                        start: idx1 as usize,
                        stop: idx2.unwrap() as usize,
                        values: new_elements,
                    },
                };
            } else {
                elements[idx1 as usize] = new_elements.remove(0);
                return PropertyNesting {
                    levels: vec![],
                    action: UpdateAction::SetElement {
                        index: idx1 as usize,
                        value: elements[idx1 as usize].clone(),
                    },
                };
            }
        }
        x => {
            println!("{:#?}", x);
            panic!();
        }
    }
}

pub fn get_nested_prop_path_helper<'argtype>(
    is_slice: bool,
    t: &'argtype ArgType,
    prop_value: &mut ArgValue<'argtype>,
    mut reader: BitReader,
) -> PropertyNesting<'argtype> {
    let cont = reader.read_u8(1).unwrap();
    if cont == 0 {
        return nested_update_command(is_slice, t, prop_value, reader);
    }
    //println!("{} {} {}", cont, prop_idx, cont2);
    match (t, prop_value) {
        (
            crate::rpc::typedefs::ArgType::FixedDict((_, propspec)),
            ArgValue::FixedDict(propvalue),
        ) => {
            let prop_idx = reader
                .read_u8(propspec.len().next_power_of_two().trailing_zeros() as u8)
                .unwrap();
            let prop_id = &propspec[prop_idx as usize].name;
            //let cont = reader.read_u8(1).unwrap();
            //println!("{:#?}", propspec[prop_idx as usize]);
            let mut nesting = get_nested_prop_path_helper(
                is_slice,
                &propspec[prop_idx as usize].prop_type,
                propvalue.get_mut(prop_id.as_str()).unwrap(),
                reader,
            );
            nesting.levels.push(PropertyNestLevel::DictKey(
                &propspec[prop_idx as usize].name,
            ));
            return nesting;
        }
        (
            crate::rpc::typedefs::ArgType::FixedDict((_, propspec)),
            ArgValue::NullableFixedDict(Some(propvalue)),
        ) => {
            let prop_idx = reader
                .read_u8(propspec.len().next_power_of_two().trailing_zeros() as u8)
                .unwrap();
            let prop_id = &propspec[prop_idx as usize].name;
            //let cont = reader.read_u8(1).unwrap();
            //println!("{:#?}", propspec[prop_idx as usize]);
            let mut nesting = get_nested_prop_path_helper(
                is_slice,
                &propspec[prop_idx as usize].prop_type,
                propvalue.get_mut(prop_id.as_str()).unwrap(),
                reader,
            );
            nesting.levels.insert(
                0,
                PropertyNestLevel::DictKey(&propspec[prop_idx as usize].name),
            );
            return nesting;
        }
        (crate::rpc::typedefs::ArgType::Array((size, element_type)), ArgValue::Array(arr)) => {
            let idx = reader
                .read_u8(arr.len().next_power_of_two().trailing_zeros() as u8)
                .unwrap();
            let mut nesting =
                get_nested_prop_path_helper(is_slice, element_type, &mut arr[idx as usize], reader);
            nesting
                .levels
                .push(PropertyNestLevel::ArrayIndex(idx as usize));
            return nesting;
        }
        x => {
            println!("{:#?}", x);
            panic!()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn slice_insert_single_into_empty() {
        let mut v: Vec<u32> = vec![];
        slice_insert(0, 0, &mut v, vec![5]);
        assert_eq!(v, vec![5]);
    }

    #[test]
    fn multi_into_empty() {
        let mut v: Vec<u32> = vec![];
        slice_insert(2, 5, &mut v, vec![5, 6, 7, 8]);
        assert_eq!(v, vec![5, 6, 7, 8]);
    }

    #[test]
    fn replace_mid_single() {
        let mut v: Vec<u32> = vec![1, 2, 3, 4, 5];
        slice_insert(2, 3, &mut v, vec![6]);
        assert_eq!(v, vec![1, 2, 6, 4, 5]);
    }

    #[test]
    fn insert_mid() {
        let mut v: Vec<u32> = vec![1, 2, 3, 4, 5];
        slice_insert(2, 2, &mut v, vec![6]);
        assert_eq!(v, vec![1, 2, 6, 3, 4, 5]);
    }

    #[test]
    fn insert_mid_partial_replace() {
        let mut v: Vec<u32> = vec![1, 2, 3, 4, 5];
        slice_insert(2, 4, &mut v, vec![6, 7, 8]);
        assert_eq!(v, vec![1, 2, 6, 7, 8, 5]);
    }

    #[test]
    fn shrink_mid_with_replace() {
        let mut v: Vec<u32> = vec![1, 2, 3, 4, 5];
        slice_insert(2, 4, &mut v, vec![6]);
        assert_eq!(v, vec![1, 2, 6, 5]);
    }

    #[test]
    fn append() {
        let mut v: Vec<u32> = vec![1, 2, 3, 4, 5];
        slice_insert(5, 12, &mut v, vec![6, 7, 8]);
        assert_eq!(v, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
