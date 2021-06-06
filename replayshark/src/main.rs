use clap::{App, Arg, SubCommand};
use std::collections::HashMap;
use std::io::Write;

use wows_replays::{parse_scripts, ErrorKind, ReplayFile};

mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn parse_replay<P: wows_replays::analyzer::AnalyzerBuilder>(
    replay: &std::path::PathBuf,
    processor: P,
) -> Result<(), wows_replays::ErrorKind> {
    let replay_file = ReplayFile::from_file(replay)?;

    let datafiles = wows_replays::version::Datafiles::new(
        std::path::PathBuf::from("versions"),
        wows_replays::version::Version::from_client_exe(&replay_file.meta.clientVersionFromExe),
    )?;
    let specs = parse_scripts(&datafiles)?;

    let version_parts: Vec<_> = replay_file.meta.clientVersionFromExe.split(",").collect();
    assert!(version_parts.len() == 4);

    let processor = processor.build(&replay_file.meta);

    // Parse packets
    let mut p = wows_replays::packet2::Parser::new(&specs);
    let mut analyzer_set = wows_replays::analyzer::AnalyzerAdapter::new(vec![processor]);
    match p.parse_packets::<wows_replays::analyzer::AnalyzerAdapter>(
        &replay_file.packet_data,
        &mut analyzer_set,
    ) {
        Ok(()) => {
            analyzer_set.finish();
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn truncate_string(s: &str, length: usize) -> &str {
    match s.char_indices().nth(length) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}

fn printspecs(specs: &Vec<wows_replays::rpc::entitydefs::EntitySpec>) {
    println!("Have {} entities", specs.len());
    for entity in specs.iter() {
        println!();
        println!(
            "{} has {} properties ({} internal) and {}/{}/{} base/cell/client methods",
            entity.name,
            entity.properties.len(),
            entity.internal_properties.len(),
            entity.base_methods.len(),
            entity.cell_methods.len(),
            entity.client_methods.len()
        );

        println!("Properties:");
        for (i, property) in entity.properties.iter().enumerate() {
            println!(
                " - {}: {} flag={:?} type={:?}",
                i, property.name, property.flags, property.prop_type
            );
        }
        println!("Internal properties:");
        for (i, property) in entity.internal_properties.iter().enumerate() {
            println!(" - {}: {} type={:?}", i, property.name, property.prop_type);
        }
        println!("Client methods:");
        for (i, method) in entity.client_methods.iter().enumerate() {
            println!(" - {}: {}", i, method.name);
            for arg in method.args.iter() {
                println!("      - {:?}", arg);
            }
        }
    }
}

enum SurveyResult {
    Blacklisted,
    /// npackets, ninvalid
    Success((usize, usize)),
    UnsupportedVersion(String),
    ParseFailure(String),
}

struct SurveyResults {
    blacklisted: usize,
    version_failures: usize,
    parse_failures: usize,
    successes: usize,
    successes_with_invalids: usize,
    total: usize,
    invalid_versions: HashMap<String, usize>,
}

impl SurveyResults {
    fn empty() -> Self {
        Self {
            blacklisted: 0,
            version_failures: 0,
            parse_failures: 0,
            successes: 0,
            successes_with_invalids: 0,
            total: 0,
            invalid_versions: HashMap::new(),
        }
    }

    fn add(&mut self, result: SurveyResult) {
        self.total += 1;
        match result {
            SurveyResult::Blacklisted => {
                self.blacklisted += 1;
            }
            SurveyResult::Success((_npacks, ninvalid)) => {
                self.successes += 1;
                if ninvalid > 0 {
                    self.successes_with_invalids += 1;
                }
            }
            SurveyResult::UnsupportedVersion(version) => {
                self.version_failures += 1;
                if !self.invalid_versions.contains_key(&version) {
                    self.invalid_versions.insert(version.clone(), 0);
                }
                *self.invalid_versions.get_mut(&version).unwrap() += 1;
            }
            SurveyResult::ParseFailure(_error) => {
                self.parse_failures += 1;
            }
        }
    }

    fn print(&self) {
        println!();
        println!("Found {} replay files", self.total);
        println!(
            "- {} ({:.0}%) were parsed",
            self.successes,
            100. * self.successes as f64 / self.total as f64
        );
        println!(
            "  - Of which {} ({:.0}%) contained invalid packets",
            self.successes_with_invalids,
            100. * self.successes_with_invalids as f64 / self.successes as f64
        );
        println!(
            "- {} ({:.0}%) had a parse error",
            self.parse_failures,
            100. * self.parse_failures as f64 / self.total as f64
        );
        println!(
            "- {} ({:.0}%) are an unrecognized version",
            self.version_failures,
            100. * self.version_failures as f64 / self.total as f64
        );
        if self.invalid_versions.len() > 0 {
            for (k, v) in self.invalid_versions.iter() {
                println!("  - Version {} appeared {} times", k, v);
            }
        }
    }
}

fn survey_file(skip_decode: bool, replay: std::path::PathBuf) -> SurveyResult {
    let filename = replay.file_name().unwrap().to_str().unwrap();
    let blacklist = [
        // This one fails to parse the initial bit
        "8654fea76d1a758ea40d",
        // Ship ID was not a U32
        "a71c42aabe17848bf618",
        "cb5b3f96018265ef8dbb",
        // Failure to parse minimap info
        "94fedfc13adc497440dc",
    ];
    let is_blacklisted = blacklist
        .iter()
        .map(|prefix| filename.contains(prefix))
        .fold(false, |a, b| a | b);
    if is_blacklisted {
        return SurveyResult::Blacklisted;
    }

    print!("Parsing {}: ", truncate_string(filename, 20));
    std::io::stdout().flush().unwrap();

    let survey_stats = std::rc::Rc::new(std::cell::RefCell::new(
        wows_replays::analyzer::survey::SurveyStats::new(),
    ));
    let survey =
        wows_replays::analyzer::survey::SurveyBuilder::new(survey_stats.clone(), skip_decode);
    match parse_replay(&std::path::PathBuf::from(replay), survey) {
        Ok(_) => {
            let stats = survey_stats.borrow();
            if stats.invalid_packets > 0 {
                println!(
                    "OK ({} packets, {} invalid)",
                    stats.total_packets, stats.invalid_packets
                );
            } else {
                println!("OK ({} packets)", stats.total_packets);
            }
            SurveyResult::Success((stats.total_packets, stats.invalid_packets))
        }
        Err(ErrorKind::UnsupportedReplayVersion(n)) => {
            println!("Unsupported version {}", n);
            SurveyResult::UnsupportedVersion(n)
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
            SurveyResult::ParseFailure(format!("{:?}", e))
        }
    }
}

fn main() {
    let replay_arg = Arg::with_name("REPLAY")
        .help("The replay file to use")
        .required(true)
        .index(1);
    let matches = App::new("World of Warships Replay Parser Utility")
        .version(built_info::GIT_VERSION.unwrap_or("undefined"))
        .author("Lane Kolbly <lane@rscheme.org>")
        .about("Parses & processes World of Warships replay files")
        .subcommand(
            SubCommand::with_name("trace")
                .about("Renders an image showing the trails of ships over the course of the game")
                .arg(
                    Arg::with_name("out")
                        .long("output")
                        .help("Output PNG file to write")
                        .takes_value(true)
                        .required(true),
                )
                .arg(replay_arg.clone()),
        )
        .subcommand(
            SubCommand::with_name("survey")
                .about("Runs the parser against a directory of replays to validate the parser")
                .arg(
                    Arg::with_name("skip-decode")
                        .long("skip-decode")
                        .help("Don't run the decoder"),
                )
                .arg(
                    Arg::with_name("REPLAYS")
                        .help("The replay files to use")
                        .required(true)
                        .multiple(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("chat")
                .about("Print the chat log of the given game")
                .arg(replay_arg.clone()),
        )
        .subcommand(
            SubCommand::with_name("summary")
                .about("Generate summary statistics of the game")
                .arg(replay_arg.clone()),
        )
        .subcommand(
            SubCommand::with_name("dump")
                .about("Dump the packets to console")
                .arg(
                    Arg::with_name("output")
                        .long("output")
                        .short("o")
                        .help("Output filename to dump to")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("no-meta")
                        .long("no-meta")
                        .help("Don't output the metadata as first line"),
                )
                .arg(replay_arg.clone()),
        )
        .subcommand(
            SubCommand::with_name("spec")
                .about("Dump the scripts specifications to console")
                .arg(
                    Arg::with_name("version")
                        .help("Version to dump. Must be comma-delimited: major,minor,patch,build")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("dump") {
        let input = matches.value_of("REPLAY").unwrap();
        /*let mut dump = PacketDump {
            time_offset: 2431.0,
        };
        parse_replay(&std::path::PathBuf::from(input), dump).unwrap();*/
        /*let mut dump = DamageMonitor {
            avatarid: 511279, //avatarid: 576297,
            shipid: 511280,   //shipid: 576298,
            time_offset: 5824.0,
            artillery_shots: HashMap::new(),
            position: (1e9, 1e9, 1e9),
            trail: vec![],
            meta: None,
            output: "foo.png".to_string(),
            damages: vec![],
        };*/
        //let dump = wows_replays::analyzer::packet_dump::PacketDumpBuilder::new(2431.0);
        //let mut dump = wows_replays::analyzer::damage_trails::DamageTrailsBuilder::new();
        let dump = wows_replays::analyzer::decoder::DecoderBuilder::new(
            false,
            matches.is_present("no-meta"),
            matches.value_of("output"),
        );
        parse_replay(&std::path::PathBuf::from(input), dump).unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("spec") {
        let datafiles = wows_replays::version::Datafiles::new(
            std::path::PathBuf::from("versions"),
            wows_replays::version::Version::from_client_exe(matches.value_of("version").unwrap()),
        )
        .unwrap();
        let specs = parse_scripts(&datafiles).unwrap();
        printspecs(&specs);
    }
    if let Some(matches) = matches.subcommand_matches("summary") {
        let input = matches.value_of("REPLAY").unwrap();
        let dump = wows_replays::analyzer::summary::SummaryBuilder::new();
        parse_replay(&std::path::PathBuf::from(input), dump).unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("chat") {
        let input = matches.value_of("REPLAY").unwrap();
        let chatlogger = wows_replays::analyzer::chat::ChatLoggerBuilder::new();
        parse_replay(&std::path::PathBuf::from(input), chatlogger).unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("trace") {
        let input = matches.value_of("REPLAY").unwrap();
        let output = matches.value_of("out").unwrap();
        let trailer = wows_replays::analyzer::trails::TrailsBuilder::new(output);
        parse_replay(&std::path::PathBuf::from(input), trailer).unwrap();
    }
    if let Some(matches) = matches.subcommand_matches("survey") {
        let mut survey_result = SurveyResults::empty();
        for replay in matches.values_of("REPLAYS").unwrap() {
            for entry in walkdir::WalkDir::new(replay) {
                let entry = entry.expect("Error unwrapping entry");
                if !entry.path().is_file() {
                    continue;
                }
                let replay = entry.path().to_path_buf();
                let result = survey_file(matches.is_present("skip-decode"), replay);
                survey_result.add(result);
            }
        }
        survey_result.print();
    }
}
