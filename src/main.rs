use nom::{bytes::complete::take, bytes::complete::tag, named, do_parse, take, tag, number::complete::be_u16, number::complete::le_u16, number::complete::be_u8, alt, cond, number::complete::be_u24, char, opt, one_of, take_while, length_data, many1, complete, number::complete::le_u32, number::complete::le_f32, multi::many0, number::complete::be_u32, multi::count};
use std::collections::HashMap;
use std::convert::TryInto;
use plotters::prelude::*;
use image::{imageops::FilterType, ImageFormat};

mod error;
mod wowsreplay;
mod packet;

use error::*;
use wowsreplay::*;
use packet::*;

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
        for _ in 0..indent {
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

fn parse_8_35_part(i: &[u8]) -> IResult<&[u8], (u32, f32)> {
    let (i, pid) = le_u32(i)?;
    let (i, damage) = le_f32(i)?;
    Ok((i, (pid, damage)))
}

fn parse_8_35(i: &[u8]) -> IResult<&[u8], Vec<(u32, f32)>> {
    let (i, cnt) = be_u8(i)?;
    let (i, data) = count(parse_8_35_part, cnt.try_into().unwrap())(i)?;
    assert!(i.len() == 0);
    Ok((i, data))
}

// From https://stackoverflow.com/questions/35901547/how-can-i-find-a-subsequence-in-a-u8-slice
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn find_float_approx(haystack: &[u8], needle: f32, epsilon: f32) -> Option<usize> {
    haystack.windows(4).position(|window| {
        let x = f32::from_le_bytes(window.try_into().unwrap());
        (x.abs() - needle).abs() <= epsilon
    })
}

fn parse_replay(replay: &std::path::PathBuf) {
    let replay_file = ReplayFile::from_file(replay);

    let root = BitMapBackend::new("test.png", (2048, 2048)).into_drawing_area();
    root.fill(&BLACK).unwrap();

    let scale = 700.0; // 570 for 36km maps
    let mut scatter_ctx = ChartBuilder::on(&root)
        .x_label_area_size(0)
        .y_label_area_size(0)
        .build_ranged(-scale..scale, -scale..scale).unwrap();

    // Search it for damage values:
    // - 1848: Damage to player
    // - 10292: Damage to Gneisenau PhantomBR_1
    //let needle = [0x34, 0x28];//, 0x00, 0x00];
    //let needle = [0xd8, 0x4, 0x0, 0x0]; // An overpen
    //let needle = [0x66, 0x1e, 0x00, 0x00]; // Enemy ship's HP (7782, VeryHonorabru)
    //let needle = [0x04, 0x74, 0x0, 0x0];
    //let needle = [0x00, 0x72, 0x17, 0x47];//, 0x00, 0x00];
    let needle = 323.0;//60650.0;//0.79122489796;//38770.0;

    // Parse packets
    let (remaining, packets) = parse_packets(&replay_file.packet_data).unwrap();
    let mut points = HashMap::new();
    let mut d0 = vec!();
    let mut d1 = vec!();
    let mut d2 = vec!();
    let mut d3 = vec!();
    let mut d4 = vec!();
    let mut d5 = vec!();
    for packet in packets.iter() {
        /*if let Some(x) = find_subsequence(packet.raw, &needle) {
            println!("Found needle subpattern!");
            hexdump::hexdump(packet.raw);
        }*/
        if let Some(x) = find_float_approx(packet.raw, needle, needle / 100.0) {
            println!("Found needle subpattern!");
            hexdump::hexdump(packet.raw);
        }
        match packet {
            Packet { clock, payload: PacketType::Position(p), .. } => {
                if !points.contains_key(&p.pid) {
                    points.insert(p.pid, vec!());
                }
                points.get_mut(&p.pid).unwrap().push((p.x as f64, p.z as f64));
            }
            Packet { clock, payload: PacketType::Entity(p), .. } => {
                if p.supertype == 0x8 {
                    if p.subtype == 0x77 {
                        println!("{}: Got {}-byte 0x8 packet subtype=0x{:x}:", clock, p.payload.len(), p.subtype);
                        //hexdump::hexdump(p.payload);
                        parse_77(p.payload);
                    } else if p.subtype == 0x76 {
                        println!("{}: Got chat packet!", clock);
                    } else if p.subtype == 0x30 {
                        println!("{}: Got 0x8 0x30 packet!", clock);
                        //hexdump::hexdump(p.payload);
                    } else if p.subtype == 0x6f {
                        // This packet appears to be:
                        // 32-bit player id
                        // 32-bit subtype (? Either 0x3 or 0x28)
                        // 8-bit "count of objects" (each object is 20 bytes)
                        // Some f32 data, I guess
                        println!("{}: Got 0x8 0x6f packet!", clock);
                        hexdump::hexdump(p.payload);
                    } else if p.subtype == 0x45 {
                        // Appears to be always the same?
                        // Player ID followed by 5 bytes
                        println!("{}: Got 0x8 0x45 packet!", clock);
                        //hexdump::hexdump(p.payload);
                        assert!(p.payload.len() == 9);
                    } else if p.subtype == 0x3c {
                        println!("{}: Got 0x8 0x3c packet!", clock);
                        hexdump::hexdump(p.payload);
                    } else if p.subtype == 0x79 {
                        println!("{}: Got 0x8 0x79 packet!", clock);
                        hexdump::hexdump(p.payload);
                    } else if p.subtype == 0x63 {
                        println!("{}: Got 0x8 0x63 packet! (volley hit?)", clock);
                        hexdump::hexdump(p.payload);
                    } else if p.subtype == 0x35 {
                        println!("{}: Got 0x8 0x35 packet! (Damage received) entity_id=0x{:x}", clock, p.entity_id);
                        hexdump::hexdump(p.payload);
                    } else if p.subtype == 0xc {
                        // This is the banners the player receives
                        // 3: Shot down plane
                        // 4: Incapacitation
                        // 6: Set fire
                        // 8: Citadel
                        // 13: Secondary hit
                        // 14: Overpenentration
                        // 15: Penetration
                        // 16: Non-penetration
                        // 17: Ricochet
                        // 28: Torpedo protection hit
                        println!("{}: Got 0x8 0xc packet! (banners) entity_id=0x{:x} data={:?}", clock, p.entity_id, p.payload);
                        assert!(p.payload.len() == 1);
                    } else {
                        println!("{}: Got {}-byte 0x8 packet subtype=0x{:x}", clock, p.payload.len(), p.subtype);
                        /*if let Some(x) = find_subsequence(p.payload, &needle) {
                            println!("Found damage subpattern!");
                            hexdump::hexdump(p.payload);
                        }*/
                    }
                } else {
                    assert!(p.supertype == 0x7);
                    println!("{}: Got {}-byte 0x7 packet subtype=0x{:x}", clock, p.payload.len(), p.subtype);
                    /*if let Some(x) = find_subsequence(p.payload, &needle) {
                        println!("Found needle subpattern!");
                        hexdump::hexdump(p.payload);
                    }*/
                }
            }
            Packet { clock, payload: PacketType::Chat(p), .. } => {
                println!("{}: Got chat packet: audience='{}' message='{}' ({:?})", clock, p.audience, p.message, p);
            }
            Packet { clock, payload: PacketType::Timing(p), .. } => {
                //println!("{}: Timing={}", clock, p.time);
            }
            Packet { clock, payload: PacketType::Type24(p), .. } => {
                println!("{:.3}: Got packet 0x24: {:?}", clock, p);
            }
            Packet { clock, payload: PacketType::Type2b(p), .. } => {
                println!("{:.3}: Got packet 0x2b: {:x?}", clock, p);
                if p.sub_object_id == 0 {
                    d0.push((*clock, p.f0));
                    d1.push((*clock, p.f1));
                    d2.push((*clock, p.f2));
                    d3.push((*clock, p.f3));
                    d4.push((*clock, p.f4));
                    d5.push((*clock, p.f5));
                }
            }
            Packet { clock, payload: PacketType::Type8_79(p), .. } => {
                println!("{:.3}: Got 0x8 0x79: {:?}", clock, p);
            }
            Packet { clock, packet_type, payload: PacketType::ArtilleryHit(p), .. } => {
                println!("{}: Got artillery packet damage={} subject=0x{:x}", clock, p.damage, p.subject);
                //println!("{:#?}", p);
            }
            Packet { clock, packet_type, payload: PacketType::Unknown(payload), .. } => {
                //_ => {
                println!("{}: Got {}-byte packet 0x{:x}", clock, payload.len(), packet_type);
                if *packet_type == 0x5 {
                    if payload[0] == 0xe {
                        println!("Maybe player ship:");
                    }
                    hexdump::hexdump(payload);
                }

                /*if let Some(x) = find_subsequence(payload, &needle) {
                    println!("Found needle subpattern!");
                    hexdump::hexdump(payload);
                }*/
            }
        }
    }

    // Blit in the map
    {
        let mut ctx = ChartBuilder::on(&root)
            .x_label_area_size(0)
            .y_label_area_size(0)
            .build_ranged(0.0..1.0, 0.0..1.0).unwrap();

        let image = image::load(std::io::BufReader::new(std::fs::File::open("320px-Fault_Line.png").unwrap()), ImageFormat::Png).unwrap().resize_exact(2048, 2048, FilterType::Nearest);
        //let image = image::load(std::io::BufReader::new(std::fs::File::open("320px-New_Dawn.png").unwrap()), ImageFormat::Png).unwrap().resize_exact(2048, 2048, FilterType::Nearest);
        let elem: BitMapElement<_> = ((0.0, 1.0), image).into();
        ctx.draw_series(std::iter::once(elem)).unwrap();
    }

    let colors = [
        BLUE,
        CYAN,
        GREEN,
        MAGENTA,
        RED,
        WHITE,
        YELLOW,
    ];
    println!("Have {} tracks", points.len());
    let mut min_x = 0.;
    let mut max_x = 0.;
    for (i,(_k,v)) in points.iter().enumerate() {
        //println!("{}", v.len());
        let series_minx = v.iter().map(|(x, _y)| x).min_by(|a, b| { a.partial_cmp(b).unwrap() }).unwrap();
        let series_maxx = v.iter().map(|(x, _y)| x).max_by(|a, b| { a.partial_cmp(b).unwrap() }).unwrap();
        if *series_minx < min_x {
            min_x = *series_minx;
        }
        if *series_maxx > max_x {
            max_x = *series_maxx;
        }
        scatter_ctx.draw_series(
            v.iter()
                .map(|(x, y)| Circle::new((*x, *y), 1, colors[i % colors.len()].filled())),
        ).unwrap();
    }
    println!("Min X: {} max X: {}", min_x, max_x);

    // Add the player position from d0/d2
    {
        let mut v = vec!();
        for idx in 0..d0.len() {
            v.push((d0[idx].1 as f64, d2[idx].1 as f64));
        }
        scatter_ctx.draw_series(
            v.iter()
                .map(|(x, y)| Circle::new((*x, *y), 2, WHITE.filled())),
        ).unwrap();
    }

    // Draw the chart
    {
        let root = BitMapBackend::new("chart.png", (1920, 1080)).into_drawing_area();
        root.fill(&WHITE).unwrap();
        let root = root.margin(10, 10, 10, 10);
        // After this point, we should be able to draw construct a chart context
        let max_x = *d5.iter().map(|(a,_b)| { a }).max_by(|a, b| { a.partial_cmp(b).unwrap() }).unwrap();
        let min_y = *d5.iter().map(|(_a,b)| { b }).min_by(|a, b| { a.partial_cmp(b).unwrap() }).unwrap();
        let max_y = *d5.iter().map(|(_a,b)| { b }).max_by(|a, b| { a.partial_cmp(b).unwrap() }).unwrap();
        let mut chart = ChartBuilder::on(&root)
        // Set the caption of the chart
            .caption("This is our first plot", ("sans-serif", 40).into_font())
        // Set the size of the label region
            .x_label_area_size(20)
            .y_label_area_size(40)
        // Finally attach a coordinate on the drawing area and make a chart context
            .build_ranged(
                0f32..max_x,
                min_y..max_y,
            ).unwrap();

        // Then we can draw a mesh
        chart
            .configure_mesh()
        // We can customize the maximum number of labels allowed for each axis
            .x_labels(5)
            .y_labels(5)
        // We can also change the format of the label text
            .y_label_formatter(&|x| format!("{:.3}", x))
            .draw().unwrap();

        chart.draw_series(LineSeries::new(
            d0,
            &RED,
        )).unwrap();

        chart.draw_series(LineSeries::new(
            d1,
            &CYAN,
        )).unwrap();

        chart.draw_series(LineSeries::new(
            d2,
            &GREEN,
        )).unwrap();

        chart.draw_series(LineSeries::new(
            d3,
            &BLUE,
        )).unwrap();

        chart.draw_series(LineSeries::new(
            d4,
            &MAGENTA,
        )).unwrap();

        chart.draw_series(LineSeries::new(
            d5,
            &BLACK,
        )).unwrap();
    }

    // Compute a histogram of packets
    let mut packet_counts = HashMap::new();
    let mut total_damage = 0;
    let mut banners = HashMap::new();
    for packet in packets.iter() {
        match packet {
            Packet { clock, packet_type, payload: PacketType::ArtilleryHit(p), .. } => {
                if !p.is_incoming {
                    total_damage += p.damage;
                }
                println!(
                    "{}: Got artillery packet: {} 0x{:x} {} {}doing {} damage{}",
                    clock,
                    if p.is_incoming { "From" } else { "To" },
                    p.subject,
                    if p.is_he { "HE" } else { "AP" },
                    if p.is_secondary { "secondary " } else { "" },
                    p.damage,
                    if p.incapacitations.len() > 0 {
                        format!(" with incapacitations={:x?}", p.incapacitations)
                    } else { "".to_string() }
                );
                println!("Bitmasks: 0x{:08x} 0x{:08x} 0x{:08x} 0x{:08x} 0x{:08x} 0x{:08x}\n", p.bitmask0, p.bitmask1, p.bitmask2, p.bitmask3, p.bitmask4, p.bitmask5);
                //println!("{:#?}", p);
            }
            Packet { clock, payload: PacketType::Entity(p), .. } => {
                if p.supertype == 0x8 {
                    if !packet_counts.contains_key(&p.subtype) {
                        packet_counts.insert(p.subtype, 0);
                    }
                    *packet_counts.get_mut(&p.subtype).unwrap() += 1;
                    if p.subtype == 0xc {
                        println!("{}: Got 0x8 0x{:x} packet! payload={:?}", clock, p.subtype, p.payload);
                        assert!(p.payload.len() == 1);
                        if !banners.contains_key(&p.payload[0]) {
                            banners.insert(p.payload[0], 0);
                        }
                        *banners.get_mut(&p.payload[0]).unwrap() += 1;
                        //hexdump::hexdump(p.payload);
                    }
                    if p.subtype == 0x35 {
                        println!("{}: Got 0x8 0x35 packet!", clock);
                        let (_, v) = parse_8_35(p.payload).unwrap();
                        /*let (i, cnt) = be_u8::<_, error::Error<&[u8]>>(p.payload).unwrap();
                        /*let parser = |i: &[u8]| -> IResult<&[u8], (u32, f32)> {
                            let (i, pid) = le_u32(i)?;
                            let (i, damage) = le_f32(i)?;
                            Ok((i, (pid, damage)))
                        };
                        let (i, data) = count(parser, cnt as usize)(i).unwrap();*/
                        let mut v = vec!();
                        let mut i = i;
                        for i in 0..cnt {
                            let (new_i, pid) = le_u32(i).unwrap();
                            let (new_i, damage) = le_f32(new_i).unwrap();
                            v.push((pid, damage));
                            i = new_i;
                        }
                        assert!(i.len() == 0);*/
                        println!("{}: Data: 0x{:x} -> {:x?}", clock, p.entity_id, v);
                        //hexdump::hexdump(p.payload);
                    }
                }
            }
            _ => {}
        }
    }
    let mut packet_counts: Vec<(_, _)> = packet_counts.iter().collect();
    packet_counts.sort();
    for (k,v) in packet_counts.iter() {
        println!("0x{:x}: {} instances", k, v);
    }
    println!("Found {} different packet types", packet_counts.len());
    println!("Player did {} damage!", total_damage);

    for (k,v) in banners.iter() {
        println!("Banner {}: {}x", k, v);
    }

    // Some debugging code
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
}

fn main() {
    parse_replay(&std::path::PathBuf::from("replays/20200605_183626_PASB008-Colorado-1945_13_OC_new_dawn.wowsreplay"));
    //parse_replay(&std::path::PathBuf::from("replays/20200620_155225_PRSD205-Podvoisky-pr-1929_17_NA_fault_line.wowsreplay"));
    //parse_replay("replays/20200605_185913_PRSB106-Izmail_08_NE_passage.wowsreplay");
    //parse_replay(&std::path::PathBuf::from("replays/20200605_112630_PASC207-Helena_10_NE_big_race.wowsreplay"));

    //let mut v = vec!();
    /*let mut paths: Vec<_> = std::fs::read_dir("replays/").unwrap().map(|e| { e.unwrap() }).collect();
    paths.sort_by(|a, b| { a.path().cmp(&b.path()) });
    for entry in paths {
        let path = entry.path();
        if !path.is_dir() {
            //println!("{:?}", path);
            let replay = ReplayFile::from_file(&path);

            println!("date={} ship={} map={} version={}", replay.meta.dateTime, replay.meta.playerVehicle, replay.meta.mapName, replay.meta.clientVersionFromExe);

            //parse_replay(&path);

            //println!("{:?} -> 0x{:x}", path, r);
            //v.push((r,s));
        }
    }*/
}
