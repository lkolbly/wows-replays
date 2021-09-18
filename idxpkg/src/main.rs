use clap::{App, Arg, ArgGroup, SubCommand};
use memmap::MmapOptions;
use nom::{
    bytes::complete::{tag, take, take_till},
    number::complete::{le_i32, le_i64, le_u64},
    IResult,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};

#[derive(Debug)]
struct Header {
    num_nodes: i32,
    num_files: i32,
    third_offset: i64,
    trailer_offset: i64,
    first_block: Vec<u8>,
    unknown1: i64,
    unknown2: i64,
}

#[derive(Debug)]
struct Node {
    name: String,
    id: u64,
    parent: u64,
    unknown: Vec<u8>,
}

#[derive(Debug, Clone)]
struct RawFileRecord {
    id: u64,
    offset: i64,
    length: i32,
    uncompressed_length: i64,
    raw: Vec<u8>,
}

fn parse_header(i: &[u8]) -> IResult<&[u8], Header> {
    let (i, _) = tag([0x49, 0x53, 0x46, 0x50])(i)?; // ISFP
    let (i, first_block) = take(12usize)(i)?;
    let (i, num_nodes) = le_i32(i)?;
    let (i, num_files) = le_i32(i)?;
    let (i, unknown1) = le_i64(i)?;
    let (i, unknown2) = le_i64(i)?;
    let (i, third_offset) = le_i64(i)?;
    let (i, trailer_offset) = le_i64(i)?;
    Ok((
        i,
        Header {
            num_nodes,
            num_files,
            third_offset,
            first_block: first_block.to_owned(),
            unknown1,
            unknown2,
            trailer_offset,
        },
    ))
}

fn parse_node(i: &[u8]) -> IResult<&[u8], (&[u8], i64, u64, u64)> {
    let (i, unknown) = take(8usize)(i)?;
    let (i, ptr) = le_i64(i)?;
    let (i, id) = le_u64(i)?;
    let (i, parent) = le_u64(i)?;
    Ok((i, (unknown, ptr, id, parent)))
}

fn parse_pointers_with_strings(num_nodes: i32, i: &[u8]) -> IResult<&[u8], Vec<Node>> {
    let mut i = i;
    let mut nodes = vec![];
    for _ in 0..num_nodes {
        let (new_i, (unknown, ptr, id, parent)) = parse_node(i)?;
        let ptr = ptr as usize;

        // The string is nul-terminated, find the nul termination
        let mut len = 0;
        loop {
            if i[ptr + len] == 0 {
                break;
            }
            len += 1;
        }
        let string = &i[ptr..ptr + len];
        let string = std::str::from_utf8(string).unwrap();
        nodes.push(Node {
            name: string.to_owned(),
            id: id,
            parent: parent,
            unknown: unknown.to_owned(),
        });

        i = new_i;
    }
    Ok((i, nodes))
}

fn parse_file_record(i: &[u8]) -> IResult<&[u8], RawFileRecord> {
    let orig_i = i;
    let (i, id) = le_u64(i)?;
    let (i, _) = take(8usize)(i)?;
    let (i, offset) = le_i64(i)?;
    let (i, _) = take(8usize)(i)?;
    let (i, length) = le_i32(i)?;
    let (i, _) = take(4usize)(i)?;
    let (i, uncompressed_length) = le_i64(i)?;
    Ok((
        i,
        RawFileRecord {
            id,
            offset,
            length,
            uncompressed_length,
            raw: orig_i.to_owned(),
        },
    ))
}

fn parse_file_records(num_files: i32, i: &[u8]) -> IResult<&[u8], Vec<RawFileRecord>> {
    let mut i = i;
    let mut records = vec![];
    for _ in 0..num_files {
        let (_new_i, record) = parse_file_record(&i[..48])?;
        records.push(record);
        i = &i[48..];
    }
    Ok((i, records))
}

fn parse_trailer(i: &[u8]) -> IResult<&[u8], String> {
    let (i, _) = le_i64(i)?;
    let (i, _) = le_i64(i)?;
    let (i, _) = le_u64(i)?;
    let (i, pkg_name) = take_till(|b| b == 0)(i)?;
    Ok((i, std::str::from_utf8(pkg_name).unwrap().to_owned()))
}

struct IdxFile {
    pkg_name: String,
    nodes: Vec<Node>,
    files: Vec<RawFileRecord>,
}

fn parse_file(i: &[u8]) -> IResult<&[u8], (String, IdxFile)> {
    let orig_i = i;
    let (i, header) = parse_header(i)?;
    let (i, nodes) = parse_pointers_with_strings(header.num_nodes, i)?;
    let (_, file_records) = parse_file_records(
        header.num_files,
        &orig_i[header.third_offset as usize + 0x10..],
    )?;
    let (_, name) = parse_trailer(&orig_i[header.trailer_offset as usize + 0x10..])?;
    Ok((
        i,
        (
            name.clone(),
            IdxFile {
                pkg_name: name.clone(),
                nodes,
                files: file_records,
            },
        ),
    ))
}

impl IdxFile {
    fn paths(&self) -> Vec<(String, RawFileRecord)> {
        let mut node_indices = std::collections::HashMap::new();
        for (idx, node) in self.nodes.iter().enumerate() {
            node_indices.insert(node.id, idx);
        }

        let mut paths = vec![];
        for file in self.files.iter() {
            let mut parts = vec![];
            let mut current_search = file.id;
            loop {
                let name = node_indices.get(&current_search);
                match name {
                    Some(idx) => {
                        let idx = *idx;
                        current_search = self.nodes[idx].parent;
                        parts.push(self.nodes[idx].name.to_string());
                    }
                    None => break,
                }
            }

            let mut path = format!("{}", parts[parts.len() - 1]);
            for i in 1..parts.len() {
                path = format!("{}/{}", path, parts[parts.len() - i - 1]);
            }
            paths.push((path, file.clone()));
        }
        paths
    }
}

#[derive(Debug)]
struct FileRecord {
    pkg_name: String,
    path: String,
    id: u64,
    offset: usize,
    length: usize,
    uncompressed_length: usize,
}

struct IdxSet {
    files: Vec<FileRecord>,
}

impl IdxSet {
    fn append(&mut self, other: &IdxFile) {
        for (path, file) in other.paths().drain(..) {
            // Check that the ID and the path are unique
            /*for existing_file in self.files.iter() {
                if existing_file.path == path || existing_file.id == file.id {
                    /*if existing_file.offset != file.offset as usize
                        || existing_file.pkg_name != other.pkg_name
                    {
                        panic!();
                    }*/

                    println!(
                        "{} {} {} {} {}",
                        path, existing_file.id, file.id, existing_file.offset, file.offset
                    );
                    panic!();
                }
                if existing_file.id == file.id {
                    //panic!();
                }
            }*/

            self.files.push(FileRecord {
                pkg_name: other.pkg_name.clone(),
                path: path,
                id: file.id,
                offset: file.offset as usize,
                length: file.length as usize,
                uncompressed_length: file.uncompressed_length as usize,
            });
        }
    }
}

/// Encapsulates everything required to interact with a idx/pkg set
struct IdxPkgManager {
    idxset: IdxSet,
    pkg_prefix: String,
    pkgs: RefCell<HashMap<String, memmap::Mmap>>,
}

impl IdxPkgManager {
    /// Creates a new resource manager from a path to the .idx files (usually
    /// <game path>/bin/<number>/idx/) and a path to the .pkg files (usually
    /// <game path>/res_packages/)
    fn new(idx_path: &str, pkg_path: &str) -> Self {
        let mut idxset = IdxSet { files: vec![] };
        for entry in walkdir::WalkDir::new(idx_path) {
            let path = entry.unwrap();
            let path = path.path();
            if !path.is_file() {
                continue;
            }

            let mut contents = vec![];
            let mut f = std::fs::File::open(&path).unwrap();
            f.read_to_end(&mut contents).unwrap();
            let (_, (_pkg_name, idx_file)) = parse_file(&contents).unwrap();

            idxset.append(&idx_file);
        }
        Self {
            idxset,
            pkg_prefix: pkg_path.to_string(),
            pkgs: RefCell::new(HashMap::new()),
        }
    }

    /// Returns an iterator over all of the FileRecords contained in this package set.
    fn iter(&self) -> std::slice::Iter<FileRecord> {
        self.idxset.files.iter()
    }

    /// Extracts the given FileRecord into an array of bytes. Note that in order to
    /// get a FileRecord, one should first call iter().
    fn extract(&self, record: &FileRecord) -> Vec<u8> {
        let mut pkgs = self.pkgs.borrow_mut();
        let mmap = if let Some(m) = pkgs.get(&record.pkg_name) {
            m
        } else {
            let pkg_path = format!("{}/{}", self.pkg_prefix, record.pkg_name);
            let f = std::fs::File::open(&pkg_path).expect(&pkg_path);
            let map = unsafe { MmapOptions::new().map(&f).unwrap() };
            pkgs.insert(record.pkg_name.clone(), map);
            pkgs.get(&record.pkg_name).unwrap()
        };

        // TODO: Determine the actual way they encode this data
        if record.length != record.uncompressed_length {
            let mut deflater = flate2::read::DeflateDecoder::new(
                &mmap[record.offset..record.offset + record.length],
            );
            let mut contents = vec![0; record.uncompressed_length];
            let res = deflater.read_exact(&mut contents);
            if !res.is_ok() {
                panic!();
            }
            contents
        } else {
            mmap[record.offset..record.offset + record.length].to_vec()
        }
    }
}

fn main() {
    let matches = App::new("World of Warships Game Resources Unpacker")
        .author("Lane Kolbly <lane@rscheme.org>")
        .about("Parses & processes World of Warships game data")
        .arg(
            Arg::with_name("WOWSPATH")
                .long("--wows-path")
                .help("Path to the World of Warships install path")
                .conflicts_with("IDXPATH")
                .conflicts_with("PKGPATH")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("BINVERSION")
                .long("--bin-version")
                .help(
                    "Specify the bin/ version number. If not specified is generated automatically.",
                )
                .takes_value(true)
                .requires("WOWSPATH"),
        )
        .arg(
            Arg::with_name("IDXPATH")
                .long("--idx-path")
                .help("Path to the .idx files")
                .requires("PKGPATH")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("PKGPATH")
                .long("--pkg-path")
                .help("Path to the .pkg files")
                .requires("IDXPATH")
                .takes_value(true),
        )
        .group(
            ArgGroup::with_name("RAWPATH")
                .arg("IDXPATH")
                .arg("PKGPATH")
                .multiple(true),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("Lists all contained resources")
                .arg(
                    Arg::with_name("test-decode")
                        .long("test-decode")
                        .help("Tests extracting each file"),
                ),
        )
        .subcommand(
            SubCommand::with_name("extract")
                .about("Extracts a resource")
                .arg(
                    Arg::with_name("PATH")
                        .help("Resource to extract")
                        .required(true),
                )
                .arg(
                    Arg::with_name("OUT")
                        .short("o")
                        .long("output")
                        .help("Output path")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .get_matches();

    let mgr = if let Some(wows_prefix) = matches.value_of("WOWSPATH") {
        let bin_version = if let Some(bin_version) = matches.value_of("BINVERSION") {
            bin_version.to_owned()
        } else {
            let bin_path = format!("{}/bin/", wows_prefix);
            let mut versions = vec![];
            for path in std::fs::read_dir(&bin_path).unwrap() {
                versions.push(
                    path.unwrap()
                        .file_name()
                        .to_str()
                        .unwrap()
                        .parse::<u32>()
                        .unwrap(),
                );
            }
            versions.sort();
            format!("{}", versions[versions.len() - 1])
        };
        let idx_prefix = format!("{}/bin/{}/idx/", wows_prefix, bin_version);
        let pkg_prefix = format!("{}/res_packages/", wows_prefix);
        IdxPkgManager::new(&idx_prefix, &pkg_prefix)
    } else {
        let idx_prefix = matches.value_of("IDXPATH").unwrap();
        let pkg_prefix = matches.value_of("PKGPATH").unwrap();
        IdxPkgManager::new(idx_prefix, pkg_prefix)
    };

    if let Some(matches) = matches.subcommand_matches("list") {
        for record in mgr.iter() {
            if matches.is_present("test-decode") {
                print!("{}... ", record.path);
                std::io::stdout().flush().unwrap();

                let contents = mgr.extract(record);

                println!("OK ({} bytes)", contents.len());
            } else {
                println!("{}", record.path);
            }
        }
    } else if let Some(matches) = matches.subcommand_matches("extract") {
        for record in mgr.iter() {
            if record.path == matches.value_of("PATH").unwrap() {
                let contents = mgr.extract(record);
                println!("Got {} bytes", contents.len());
                let mut f = std::fs::File::create(matches.value_of("OUT").unwrap()).unwrap();
                f.write_all(&contents).unwrap();
                break;
            }
        }
    }
}
