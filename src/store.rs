use crate::model::Album;
use crate::util;

use std::result::Result;

use serde_json;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;

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

        let file = File::create(&tmppath)?;
        serde_json::to_writer_pretty(file, &db)?;
        std::fs::rename(tmppath, &self.filename)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{Album, Track};
    use crate::youtube;
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
        //println!("store path: {:?}", tmppath);

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
                video_file: None,
                youtube_id: Some(youtube::VideoID("asdf".to_string())),
            }],
            youtube_id: Some(youtube::PlaylistID("PL0123".to_string())),
        };
        store.save(&album).unwrap();
        let a = store
            .get_album("https://ektoplazm.com/free-music/globular-entangled-everything")
            .unwrap();
        assert_eq!(Some(album), a);
    }
}
