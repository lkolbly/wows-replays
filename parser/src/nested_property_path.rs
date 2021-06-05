use crate::rpc::typedefs::{ArgType, ArgValue};
use bitreader::BitReader;

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
) {
    match (t, &mut prop_value) {
        (ArgType::FixedDict((_, entries)), _) => {
            let entry_idx = reader
                .read_u8(entries.len().next_power_of_two().trailing_zeros() as u8)
                .unwrap();
            println!("{}: {:#?}", entry_idx, entries[entry_idx as usize]);
            while reader.remaining() % 8 != 0 {
                reader.read_u8(1).unwrap();
            }
            let mut remaining = vec![0; reader.remaining() as usize / 8];
            reader.read_u8_slice(&mut remaining[..]).unwrap();
            assert!(reader.remaining() == 0);
            println!("{:?}", remaining);
            let value = entries[entry_idx as usize]
                .prop_type
                .parse_value(&remaining[..])
                .unwrap();
            println!("New value: {:#?}", value);
        }
        (ArgType::Array((_size, element_type)), ArgValue::Array(ref mut elements)) => {
            println!("{:#?}", elements);
            println!("{:#?}", element_type);
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

            println!("{}", reader.remaining());
            while reader.remaining() % 8 != 0 {
                reader.read_u8(1).unwrap();
            }
            println!("{}", reader.remaining());
            let mut remaining = vec![0; reader.remaining() as usize / 8];
            reader.read_u8_slice(&mut remaining[..]).unwrap();
            println!("{:?}", remaining);

            if remaining.len() == 0 {
                // Remove elements
                if is_slice {
                    slice_insert(idx1 as usize, idx2.unwrap() as usize, elements, vec![]);
                    return;
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

            println!("{:#?}", new_elements);

            if is_slice {
                println!("indices: {}-{}", idx1, idx2.unwrap());
                slice_insert(
                    idx1 as usize,
                    idx2.unwrap() as usize,
                    elements,
                    new_elements,
                );
                println!("{:#?}", elements);
            } else {
                elements[idx1 as usize] = new_elements.remove(0);
                //unimplemented!();
                println!("{:#?}", elements);
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
) {
    let cont = reader.read_u8(1).unwrap();
    if cont == 0 {
        println!("Remaining: {}", reader.remaining());
        nested_update_command(is_slice, t, prop_value, reader);
        //panic!();
        return;
    }
    //println!("{} {} {}", cont, prop_idx, cont2);
    match (t, prop_value) {
        (
            crate::rpc::typedefs::ArgType::FixedDict((_, propspec)),
            ArgValue::FixedDict(propvalue),
        ) => {
            println!(
                "{} {}",
                propspec.len(),
                propspec.len().next_power_of_two().trailing_zeros()
            );
            let prop_idx = reader
                .read_u8(propspec.len().next_power_of_two().trailing_zeros() as u8)
                .unwrap();
            let prop_id = &propspec[prop_idx as usize].name;
            //let cont = reader.read_u8(1).unwrap();
            println!("{} {}", prop_idx, cont);
            //println!("{:#?}", propspec[prop_idx as usize]);
            get_nested_prop_path_helper(
                is_slice,
                &propspec[prop_idx as usize].prop_type,
                propvalue.get_mut(prop_id.as_str()).unwrap(),
                reader,
            );
        }
        (
            crate::rpc::typedefs::ArgType::FixedDict((_, propspec)),
            ArgValue::NullableFixedDict(Some(propvalue)),
        ) => {
            println!(
                "{} {}",
                propspec.len(),
                propspec.len().next_power_of_two().trailing_zeros()
            );
            let prop_idx = reader
                .read_u8(propspec.len().next_power_of_two().trailing_zeros() as u8)
                .unwrap();
            let prop_id = &propspec[prop_idx as usize].name;
            //let cont = reader.read_u8(1).unwrap();
            println!("{} {}", prop_idx, cont);
            //println!("{:#?}", propspec[prop_idx as usize]);
            get_nested_prop_path_helper(
                is_slice,
                &propspec[prop_idx as usize].prop_type,
                propvalue.get_mut(prop_id.as_str()).unwrap(),
                reader,
            );
        }
        (crate::rpc::typedefs::ArgType::Array((size, element_type)), ArgValue::Array(arr)) => {
            println!("# of elements: {} ({:?})", arr.len(), size);
            let idx = reader
                .read_u8(arr.len().next_power_of_two().trailing_zeros() as u8)
                .unwrap();
            println!("Array idx: {}", idx);
            get_nested_prop_path_helper(is_slice, element_type, &mut arr[idx as usize], reader);
            return;
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
