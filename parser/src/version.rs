use crate::error::ErrorKind;
use rust_embed::RustEmbed;
use serde::Serialize;
use std::borrow::Cow;
use std::path::PathBuf;

#[derive(Debug, Serialize, Clone, Copy)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: u32,
}

impl Version {
    pub fn from_client_exe(version: &str) -> Version {
        let parts: Vec<_> = version.split(",").collect();
        assert!(parts.len() == 4);
        Version {
            major: parts[0].parse::<u32>().unwrap(),
            minor: parts[1].parse::<u32>().unwrap(),
            patch: parts[2].parse::<u32>().unwrap(),
            build: parts[3].parse::<u32>().unwrap(),
        }
    }

    pub fn to_path(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    pub fn is_at_least(&self, other: &Version) -> bool {
        if self.major < other.major {
            false
        } else if self.minor < other.minor {
            false
        } else if self.patch < other.patch {
            false
        } else {
            true
        }
    }
}

#[derive(RustEmbed)]
#[folder = "../versions/"]
struct Embedded;

pub struct Datafiles {
    base_path: PathBuf,
    version: Version,
}

impl Datafiles {
    pub fn new(base: PathBuf, version: Version) -> Result<Datafiles, ErrorKind> {
        let mut p = base.clone();
        p.push(version.to_path());
        // TODO: Also check the Embedded struct for if this path exists
        /*if !p.exists() {
            Err(ErrorKind::UnsupportedReplayVersion(version.to_path()))
        } else {*/
        Ok(Datafiles {
            base_path: base,
            version,
        })
        //}
    }

    pub fn get(&self, path: &str) -> Result<Cow<'static, [u8]>, ErrorKind> {
        let mut p = self.base_path.clone();
        p.push(self.version.to_path());
        p.push(path);
        if !p.exists() {
            let p = format!("{}/{}", self.version.to_path(), path);
            if let Some(x) = Embedded::get(&p) {
                return Ok(x.data);
            }
            return Err(ErrorKind::DatafileNotFound {
                version: self.version,
                path: path.to_string(),
            });
        }
        Ok(Cow::from(std::fs::read(p).unwrap()))
    }
}
