use crate::util;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::result::Result;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Album {
    #[serde(skip)]
    pub url: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>, // track id3, incl. VA
    pub title: String, // track id3
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>, // web

    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>, // track id3

    // Ektoplazm releases sometimes have multiple labels
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>, // web
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>, // web

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<Track>, // web
}

impl Album {
    pub fn dirname(&self, base_dir: &Path) -> PathBuf {
        let mut res = PathBuf::from(base_dir);

        let mut comp = match &self.artist {
            None => "VA".to_string(),
            Some(a) => a.replace(" ", "_"),
        };
        comp.push_str("_-_");
        comp.push_str(&self.title.replace(" ", "_"));

        res.push(comp);
        res
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub artist: String,
    pub title: String,

    // Single BPM for simplicity even though there are tracks w/ something like "109/175/219"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bpm: Option<u16>,

    // relative to mp3_subdir
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mp3_file: Option<PathBuf>,
}

#[derive(Debug)]
pub struct Store {
    filename: PathBuf,
}

type DB = HashMap<String, Album>;

impl Store {
    pub fn new(path: PathBuf) -> Store {
        Store { filename: path }
    }

    pub fn get_album(&self, id: &str) -> Result<Option<Album>, util::Error> {
        let db = self.read_db()?;
        let ov = db.get(id);

        match ov {
            None => Ok(None),
            Some(a) => {
                let mut aa = a.clone();
                aa.url = id.to_string();
                Ok(Some(aa))
            }
        }
    }

    pub fn save(&self, album: &Album) -> Result<(), util::Error> {
        let mut db = self.read_db()?;
        db.insert(album.url.clone(), album.clone());
        self.write_db(db)?;
        Ok(())
    }

    fn read_db(&self) -> io::Result<DB> {
        let file = File::open(&self.filename);
        if let Err(e) = file {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(DB::new());
            } else {
                return Err(e);
            }
        }
        let file = file.unwrap();
        let reader = BufReader::new(file);
        let db: DB = serde_json::from_reader(reader)?;

        Ok(db)
    }

    fn write_db(&self, db: DB) -> io::Result<()> {
        let mut tmppath = self.filename.clone();
        if !tmppath.set_extension("json-new") {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "failed to come up with temporary DB filename for {:?}",
                    self.filename
                ),
            ));
        }

        log::debug!("writing db to temporary file {:?}", tmppath);
        let file = File::create(&tmppath)?;
        serde_json::to_writer_pretty(file, &db)?;

        log::debug!("renaming temporary file to {:?}", self.filename);
        std::fs::rename(tmppath, &self.filename)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile;

    #[test]
    fn simple_roundtrip() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write(b"{}").unwrap();
        let tmppath = tmp.path();
        //let (tmp, tmppath) = tmp.keep().unwrap();
        let store = Store::new(tmppath.to_path_buf());
        println!("store path: {:?}", tmppath);

        let album = Album {
            url: "https://ektoplazm.com/free-music/globular-entangled-everything".to_string(),
            artist: Some("Globular".to_string()),
            title: "Entangled Everything".to_string(),
            license: Some("https://creativecommons.org/licenses/by-nc-sa/4.0/".to_string()),
            year: Some(2019),
            labels: vec![],
            tags: vec!["Downtempo".to_string(), "Psy Dub".to_string()],
            tracks: vec![Track {
                artist: "Globular".to_string(),
                title: "🍣".to_string(),
                bpm: Some(666),
                mp3_file: None,
            }],
        };
        store.save(&album).unwrap();
        let a = store
            .get_album("https://ektoplazm.com/free-music/globular-entangled-everything")
            .unwrap();
        assert_eq!(Some(album), a);
    }
}
