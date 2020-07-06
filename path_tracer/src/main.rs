use image::GenericImageView;
use image::Pixel;
use image::{imageops::FilterType, ImageFormat, RgbImage};
use plotters::prelude::*;
use std::collections::HashMap;

use wows_replays::{parse_packets, Packet, PacketType, ReplayFile};

fn parse_replay(replay: &std::path::PathBuf, output_image: &str) {
    let replay_file = ReplayFile::from_file(replay);

    let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
    assert!(version_parts.len() == 4);
    let build: u32 = version_parts[3].parse().unwrap();
    println!("File build version: {}", build);

    let root = BitMapBackend::new(output_image, (2048, 2048)).into_drawing_area();
    root.fill(&BLACK).unwrap();

    // 600 for New Dawn (36x36km)
    // 700 for Fault Line (42x42km)
    let scale = 600.0;

    let mut scatter_ctx = ChartBuilder::on(&root)
        .x_label_area_size(0)
        .y_label_area_size(0)
        .build_ranged(-scale..scale, -scale..scale)
        .unwrap();

    // Parse packets
    let (_, packets) = parse_packets(build, &replay_file.packet_data).unwrap();
    let mut points = HashMap::new();
    let mut player_track = vec![];
    for packet in packets.iter() {
        match packet {
            Packet {
                payload: PacketType::Position(p),
                ..
            } => {
                if !points.contains_key(&p.pid) {
                    points.insert(p.pid, vec![]);
                }
                points
                    .get_mut(&p.pid)
                    .unwrap()
                    .push((p.x as f64, p.z as f64));
            }
            Packet {
                payload: PacketType::PlayerOrientation(p),
                ..
            } => {
                if p.parent_id == 0 {
                    player_track.push((p.x as f64, p.z as f64));
                }
            }
            _ => {}
        }
    }

    // Blit in the map
    {
        //let minimap = image::load(std::io::BufReader::new(std::fs::File::open("res_unpack/spaces/17_NA_fault_line/minimap.png").unwrap()), ImageFormat::Png).unwrap();
        let minimap = image::load(
            std::io::BufReader::new(
                std::fs::File::open("res_unpack/spaces/13_OC_new_dawn/minimap.png").unwrap(),
            ),
            ImageFormat::Png,
        )
        .unwrap();
        let minimap_background = image::load(
            std::io::BufReader::new(
                std::fs::File::open("res_unpack/spaces/13_OC_new_dawn/minimap_water.png").unwrap(),
            ),
            ImageFormat::Png,
        )
        .unwrap();

        let mut image = RgbImage::new(760, 760);
        for x in 0..760 {
            for y in 0..760 {
                let bg = minimap_background.get_pixel(x, y);
                let fg = minimap.get_pixel(x, y);
                let mut bg = bg.clone();
                bg.blend(&fg);
                image.put_pixel(x, y, bg.to_rgb());
            }
        }
        let image = image::DynamicImage::ImageRgb8(image);
        let image = image.resize_exact(2048, 2048, FilterType::Lanczos3);

        let mut ctx = ChartBuilder::on(&root)
            .x_label_area_size(0)
            .y_label_area_size(0)
            .build_ranged(0.0..1.0, 0.0..1.0)
            .unwrap();

        let elem: BitMapElement<_> = ((0.0, 1.0), image).into();
        ctx.draw_series(std::iter::once(elem)).unwrap();
    }

    let colors = [BLUE, CYAN, GREEN, MAGENTA, RED, WHITE, YELLOW];
    for (i, (_k, v)) in points.iter().enumerate() {
        scatter_ctx
            .draw_series(
                v.iter()
                    .map(|(x, y)| Circle::new((*x, *y), 1, colors[i % colors.len()].filled())),
            )
            .unwrap();
    }

    // Add the player position from d0/d2
    scatter_ctx
        .draw_series(
            player_track
                .iter()
                .map(|(x, y)| Circle::new((*x, *y), 2, WHITE.filled())),
        )
        .unwrap();
}

fn main() {
    parse_replay(
        &std::path::PathBuf::from(
            "replays/20200605_183626_PASB008-Colorado-1945_13_OC_new_dawn.wowsreplay",
        ),
        "test.png",
    );
}
