use crate::analyzer::*;
use crate::packet2::{Packet, PacketType};
use crate::ReplayMeta;
use image::GenericImageView;
use image::Pixel;
use image::{imageops::FilterType, ImageFormat, RgbImage};
use plotters::prelude::*;
use std::collections::HashMap;

pub struct TrailsBuilder {
    output: String,
}

impl TrailsBuilder {
    pub fn new(output: &str) -> Self {
        Self {
            output: output.to_string(),
        }
    }
}

impl AnalyzerBuilder for TrailsBuilder {
    fn build(&self, meta: &crate::ReplayMeta) -> Box<dyn Analyzer> {
        Box::new(TrailRenderer {
            trails: HashMap::new(),
            player_trail: vec![],
            output: self.output.clone(),
            meta: Some((*meta).clone()),
        })
    }
}

struct TrailRenderer {
    trails: HashMap<u32, Vec<(f32, f32)>>,
    player_trail: Vec<(f32, f32)>,
    output: String,
    meta: Option<ReplayMeta>,
}

impl Analyzer for TrailRenderer {
    fn process(&mut self, packet: &Packet<'_, '_>) {
        match &packet.payload {
            PacketType::Position(pos) => {
                if !self.trails.contains_key(&pos.pid) {
                    self.trails.insert(pos.pid, vec![]);
                }
                self.trails.get_mut(&pos.pid).unwrap().push((pos.x, pos.z));
            }
            PacketType::PlayerOrientation(pos) => {
                self.player_trail.push((pos.x, pos.z));
            }
            _ => {}
        }
    }

    fn finish(&self) {
        // Setup the render context
        let root = BitMapBackend::new(&self.output, (2048, 2048)).into_drawing_area();
        root.fill(&BLACK).unwrap();

        // Blit the background into the image
        {
            let minimap = image::load(
                std::io::BufReader::new(
                    std::fs::File::open(&format!(
                        "versions/0.10.3/{}/minimap.png",
                        self.meta.as_ref().unwrap().mapName
                    ))
                    .unwrap(),
                ),
                ImageFormat::Png,
            )
            .unwrap();
            let minimap_background = image::load(
                std::io::BufReader::new(
                    std::fs::File::open(&format!(
                        "versions/0.10.3/{}/minimap_water.png",
                        self.meta.as_ref().unwrap().mapName
                    ))
                    .unwrap(),
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

        // Render the actual trails

        let mut map_widths: HashMap<String, u32> = HashMap::new();
        map_widths.insert("spaces/34_OC_islands".to_string(), 24);
        map_widths.insert("spaces/33_new_tierra".to_string(), 24);
        map_widths.insert("spaces/01_solomon_islands".to_string(), 30);
        map_widths.insert("spaces/10_NE_big_race".to_string(), 30);
        map_widths.insert("spaces/04_Archipelago".to_string(), 30);
        map_widths.insert("spaces/05_Ring".to_string(), 36);
        map_widths.insert("spaces/08_NE_passage".to_string(), 36);
        map_widths.insert("spaces/13_OC_new_dawn".to_string(), 36);
        map_widths.insert("spaces/17_NA_fault_line".to_string(), 42);
        map_widths.insert("spaces/41_Conquest".to_string(), 42);
        map_widths.insert("spaces/46_Estuary".to_string(), 42);
        map_widths.insert("spaces/42_Neighbors".to_string(), 42);
        map_widths.insert("spaces/50_Gold_harbor".to_string(), 42);
        map_widths.insert("spaces/20_NE_two_brothers".to_string(), 42);
        map_widths.insert("spaces/16_OC_bees_to_honey".to_string(), 48);
        map_widths.insert("spaces/22_tierra_del_fuego".to_string(), 48);
        map_widths.insert("spaces/15_NE_north".to_string(), 48);
        map_widths.insert("spaces/35_NE_north_winter".to_string(), 48);
        map_widths.insert("spaces/53_Shoreside".to_string(), 42);
        map_widths.insert("spaces/23_Shards".to_string(), 42);
        map_widths.insert("spaces/19_OC_prey".to_string(), 42);
        map_widths.insert("spaces/52_Britain".to_string(), 42);
        map_widths.insert("spaces/40_Okinawa".to_string(), 42);
        map_widths.insert("spaces/18_NE_ice_islands".to_string(), 42);
        map_widths.insert("spaces/14_Atlantic".to_string(), 42);
        map_widths.insert("spaces/38_Canada".to_string(), 48);
        map_widths.insert("spaces/37_Ridge".to_string(), 48);
        map_widths.insert("spaces/44_Path_warrior".to_string(), 48);
        map_widths.insert("spaces/25_sea_hope".to_string(), 48);
        map_widths.insert("spaces/45_Zigzag".to_string(), 48);
        map_widths.insert("spaces/47_Sleeping_Giant".to_string(), 48);
        map_widths.insert("spaces/51_Greece".to_string(), 42);
        map_widths.insert("spaces/28_naval_mission".to_string(), 42);
        map_widths.insert("spaces/00_CO_ocean".to_string(), 36);

        // 600 for New Dawn (36x36km)
        // 700 for Fault Line (42x42km)
        let scale = map_widths
            .get(&self.meta.as_ref().unwrap().mapName)
            .expect(&format!(
                "Could not find size of map {}!",
                self.meta.as_ref().unwrap().mapName
            ))
            * 50
            / 3;
        let scale = scale as f64;
        let mut scatter_ctx = ChartBuilder::on(&root)
            .x_label_area_size(0)
            .y_label_area_size(0)
            .build_ranged(-scale..scale, -scale..scale)
            .unwrap();

        let colors = [BLUE, CYAN, GREEN, MAGENTA, RED, WHITE, YELLOW];
        let mut min_x = 0.;
        let mut max_x = 0.;
        for (i, (_k, v)) in self.trails.iter().enumerate() {
            //println!("{}", v.len());
            let series_minx = v
                .iter()
                .map(|(x, _y)| x)
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap();
            let series_maxx = v
                .iter()
                .map(|(x, _y)| x)
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap();
            if *series_minx < min_x {
                min_x = *series_minx;
            }
            if *series_maxx > max_x {
                max_x = *series_maxx;
            }
            scatter_ctx
                .draw_series(v.iter().map(|(x, y)| {
                    Circle::new((*x as f64, *y as f64), 1, colors[i % colors.len()].filled())
                }))
                .unwrap();
        }

        // Add the trail for the player
        scatter_ctx
            .draw_series(
                self.player_trail
                    .iter()
                    .map(|(x, y)| Circle::new((*x as f64, *y as f64), 2, WHITE.filled())),
            )
            .unwrap();
    }
}
