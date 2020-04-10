use crate::model::{Album, Track};
use crate::util;
use crate::youtube;

use rusqlite::OptionalExtension;

use std::result::Result;

use serde_json;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub trait Store: Sized {
    fn open(path: &Path) -> Result<Self, util::Error>;
    fn get_album(&mut self, id: &str) -> Result<Option<Album>, util::Error>;
    fn save(&mut self, album: &Album) -> Result<(), util::Error>;
}

pub struct SqliteStore {
    conn: rusqlite::Connection,
}

impl rusqlite::types::ToSql for youtube::PlaylistID {
    fn to_sql(&self) -> Result<rusqlite::types::ToSqlOutput, rusqlite::Error> {
        Ok(rusqlite::types::ToSqlOutput::from(self.0.clone()))
    }
}

impl rusqlite::types::FromSql for youtube::PlaylistID {
    fn column_result(value: rusqlite::types::ValueRef) -> rusqlite::types::FromSqlResult<Self> {
        String::column_result(value).map(|s| youtube::PlaylistID(s))
    }
}

impl rusqlite::types::ToSql for youtube::VideoID {
    fn to_sql(&self) -> Result<rusqlite::types::ToSqlOutput, rusqlite::Error> {
        Ok(rusqlite::types::ToSqlOutput::from(self.0.clone()))
    }
}

impl rusqlite::types::FromSql for youtube::VideoID {
    fn column_result(value: rusqlite::types::ValueRef) -> rusqlite::types::FromSqlResult<Self> {
        String::column_result(value).map(|s| youtube::VideoID(s))
    }
}

impl Store for SqliteStore {
    fn open(path: &Path) -> Result<SqliteStore, util::Error> {
        let conn = rusqlite::Connection::open(path)?;

        conn.pragma_update(None, "foreign_keys", &"on")?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS album (
                id         INTEGER PRIMARY KEY,
                url        TEXT UNIQUE NOT NULL,
                artist     TEXT,
                title      TEXT NOT NULL,
                license    TEXT,
                year       INTEGER,
                labels     TEXT NOT NULL,
                tags       TEXT NOT NULL,
                youtube_id TEXT
             )",
            rusqlite::NO_PARAMS,
        )?;

        // AUTOINCREMENT is needed because we need the ids to be increasing to keep
        // the tracks in their album order, see: https://www.sqlite.org/autoinc.html
        conn.execute(
            "CREATE TABLE IF NOT EXISTS track (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                album_id   INTEGER NOT NULL REFERENCES album(id),
                artist     TEXT NOT NULL,
                title      TEXT NOT NULL,
                bpm        INTEGER,
                mp3_file   TEXT,
                video_file TEXT,
                youtube_id TEXT
             )",
            rusqlite::NO_PARAMS,
        )?;

        Ok(SqliteStore { conn: conn })
    }

    fn get_album(&mut self, url: &str) -> Result<Option<Album>, util::Error> {
        let tx = self.conn.transaction()?;

        let mut stmt = tx.prepare(
            "SELECT id, artist, title, license, year, labels, tags, youtube_id
             FROM album
             WHERE url = ?1",
        )?;
        let it = stmt.query_and_then::<(u32, Album), util::Error, _, _>(&[url], |row| {
            Ok((
                row.get(0)?,
                Album {
                    url: url.to_string(),
                    artist: row.get(1)?,
                    title: row.get(2)?,
                    license: row.get(3)?,
                    year: row.get(4)?,
                    labels: serde_json::from_value(row.get(5)?)?,
                    tags: serde_json::from_value(row.get(6)?)?,
                    tracks: vec![],
                    youtube_id: row.get(7)?,
                },
            ))
        })?;

        let (album_id, mut album) = match at_most_one(it)? {
            None => return Ok(None),
            Some(a) => a?,
        };

        let mut stmt = tx.prepare(
            "SELECT artist, title, bpm, mp3_file, video_file, youtube_id
             FROM track
             WHERE album_id = ?1
             ORDER BY id",
        )?;
        let it = stmt.query_map(&[album_id], |row| {
            Ok(Track {
                artist: row.get(0)?,
                title: row.get(1)?,
                bpm: row.get(2)?,
                mp3_file: row.get::<_, Option<String>>(3)?.map(|s| PathBuf::from(s)),
                video_file: row.get::<_, Option<String>>(4)?.map(|s| PathBuf::from(s)),
                youtube_id: row.get(5)?,
            })
        })?;

        album.tracks = it.collect::<Result<Vec<_>, _>>()?;

        Ok(Some(album))
    }

    fn save(&mut self, album: &Album) -> Result<(), util::Error> {
        let tx = self.conn.transaction()?;

        let res: Option<i64> = tx
            .query_row("SELECT id FROM album WHERE url = ?1", &[&album.url], |row| {
                row.get(0)
            })
            .optional()?;
        if let Some(album_id) = res {
            tx.execute("DELETE FROM track WHERE album_id = ?1", &[album_id])?;
        }

        tx.execute(
            "INSERT OR REPLACE
             INTO album (url, artist, title, license, year, labels, tags, youtube_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                album.url,
                album.artist,
                album.title,
                album.license,
                album.year,
                serde_json::to_value(&album.labels)?,
                serde_json::to_value(&album.tags)?,
                album.youtube_id
            ],
        )?;
        let album_id = tx.last_insert_rowid();

        let mut stmt = tx.prepare(
            "INSERT INTO track (album_id, artist, title, bpm, mp3_file, video_file, youtube_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for t in &album.tracks {
            stmt.execute(params![
                album_id,
                t.artist,
                t.title,
                t.bpm,
                //t.mp3_file.and_then(|f| f.to_str().as_string()),
                t.mp3_file.as_ref().and_then(|f| f.to_str().map(|s| String::from(s))),
                t.video_file.as_ref().and_then(|f| f.to_str().map(|s| String::from(s))),
                //t.video_file.and_then(|f| f.to_str().map(|s| s.as_string())),
                t.youtube_id,
            ])?;
        }
        drop(stmt);

        tx.commit()?;
        Ok(())
    }
}

fn at_most_one<T: Iterator>(mut it: T) -> Result<Option<T::Item>, util::Error> {
    let first = match it.next() {
        None => return Ok(None),
        Some(a) => a,
    };

    if let Some(_) = it.next() {
        return Err(util::Error::new("Query returned more than one result"));
    };

    Ok(Some(first))
}

#[derive(Debug)]
pub struct JsonStore {
    filename: PathBuf,
}

type DB = HashMap<String, Album>;

impl JsonStore {
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

impl Store for JsonStore {
    fn open(path: &Path) -> Result<JsonStore, util::Error> {
        //tmp.write(b"{}").unwrap();
        Ok(JsonStore {
            filename: PathBuf::from(path),
        })
    }

    fn get_album(&mut self, id: &str) -> Result<Option<Album>, util::Error> {
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

    fn save(&mut self, album: &Album) -> Result<(), util::Error> {
        let mut db = self.read_db()?;
        db.insert(album.url.clone(), album.clone());
        self.write_db(db)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Album, Track};
    use crate::youtube;
    use std::io::Write;
    use tempfile;

    fn simple_roundtrip<T: Store>() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        //tmp.write(b"{}").unwrap();
        let tmppath = tmp.path();
        std::fs::remove_file(tmppath).unwrap(); //we just need a path to nonexistent file

        let (tmp, tmppath) = tmp.keep().unwrap();
        println!("store path: {:?}", tmppath);

        let mut store = T::open(&tmppath).unwrap();

        let album_url = "https://ektoplazm.com/free-music/globular-entangled-everything";
        let a = store.get_album(album_url).unwrap();
        assert_eq!(None, a);

        let album = Album {
            url: album_url.to_string(),
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
        let a = store.get_album(album_url).unwrap();
        assert_eq!(album, a.unwrap());
        let b = store.get_album(album_url).unwrap();
        assert_eq!(album, b.unwrap());

        let mut store2 = T::open(&tmppath).unwrap();
        let c = store2.get_album(album_url).unwrap();
        assert_eq!(album, c.unwrap());

        let mut album = album;
        album.youtube_id = None;
        album.tracks.push(Track {
            artist: "Globular".to_string(),
            title: "some other trak".to_string(),
            bpm: Some(1),
            mp3_file: Some(PathBuf::from("/tmp/2.mp3")),
            video_file: Some(PathBuf::from("/tmp/2.avi")),
            youtube_id: Some(youtube::VideoID("3e4nQTFhieo".to_string())),
        });
        store.save(&album).unwrap();
        let d = store.get_album(album_url).unwrap();
        assert_eq!(album, d.unwrap());
    }

    #[test]
    fn simple_roundtrip_json() {
        simple_roundtrip::<JsonStore>()
    }

    #[test]
    fn simple_roundtrip_sqlite() {
        simple_roundtrip::<SqliteStore>()
    }
}
