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
        if self.major > other.major {
            true
        } else if self.major < other.major {
            false
        } else if self.minor > other.minor {
            true
        } else if self.minor < other.minor {
            false
        } else if self.patch >= other.patch {
            true
        } else {
            false
        }
    }
}

#[derive(RustEmbed)]
#[folder = "../versions/"]
struct Embedded;

pub trait DataFileLoader {
    fn get(&self, path: &str) -> Result<Cow<'static, [u8]>, ErrorKind>;
}

pub struct DataFileWithCallback<F> {
    callback: F,
}

impl<F> DataFileWithCallback<F>
where
    F: Fn(&str) -> Result<Cow<'static, [u8]>, ErrorKind>,
{
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<F> DataFileLoader for DataFileWithCallback<F>
where
    F: Fn(&str) -> Result<Cow<'static, [u8]>, ErrorKind>,
{
    fn get(&self, path: &str) -> Result<Cow<'static, [u8]>, ErrorKind> {
        (self.callback)(path)
    }
}

pub struct EmbeddedDataFiles {
    base_path: PathBuf,
    version: Version,
}

impl EmbeddedDataFiles {
    pub fn new(base: PathBuf, version: Version) -> Result<EmbeddedDataFiles, ErrorKind> {
        let mut p = base.clone();
        p.push(version.to_path());
        // TODO: Also check the Embedded struct for if this path exists
        /*if !p.exists() {
            Err(ErrorKind::UnsupportedReplayVersion(version.to_path()))
        } else {*/
        Ok(EmbeddedDataFiles {
            base_path: base,
            version,
        })
        //}
    }
}

impl DataFileLoader for EmbeddedDataFiles {
    fn get(&self, path: &str) -> Result<Cow<'static, [u8]>, ErrorKind> {
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

#[cfg(test)]
mod test {
    use super::*;

    fn assert_older_newer(older: Version, newer: Version) {
        assert!(newer.is_at_least(&older));
        assert!(newer.is_at_least(&newer));
        assert!(!older.is_at_least(&newer));
    }

    #[test]
    fn different_patch() {
        let older = Version::from_client_exe("0,10,9,0");
        let newer = Version::from_client_exe("0,10,10,0");
        assert_older_newer(older, newer);
    }

    #[test]
    fn different_minor() {
        let older = Version::from_client_exe("0,10,9,0");
        let newer = Version::from_client_exe("0,11,0,0");
        assert_older_newer(older, newer);
    }

    #[test]
    fn different_major() {
        let older = Version::from_client_exe("0,11,5,0");
        let newer = Version::from_client_exe("1,0,0,0");
        assert_older_newer(older, newer);
    }
}
