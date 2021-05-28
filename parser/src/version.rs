use crate::error::Error;
use std::path::PathBuf;

pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    pub fn from_client_exe(version: &str) -> Version {
        let parts: Vec<_> = version.split(",").collect();
        assert!(parts.len() == 4);
        Version {
            major: parts[0].parse::<u32>().unwrap(),
            minor: parts[1].parse::<u32>().unwrap(),
            patch: parts[2].parse::<u32>().unwrap(),
        }
    }

    fn to_path(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

pub struct Datafiles {
    base_path: PathBuf,
    version: Version,
}

impl Datafiles {
    pub fn new(base: PathBuf, version: Version) -> Datafiles {
        Datafiles {
            base_path: base,
            version,
        }
    }

    pub fn lookup(&self, path: &str) -> PathBuf {
        let mut p = self.base_path.clone();
        p.push(self.version.to_path());
        p.push(path);
        if !p.exists() {
            panic!(
                "Could not find file {} for version {}",
                path,
                self.version.to_path()
            );
        }
        p
    }
}
