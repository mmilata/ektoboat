use crate::model::*;
use crate::util;

use std::collections::VecDeque;
use std::fmt::Write;
use std::io::{copy, Read, Seek};
use std::path::{Path, PathBuf};
use std::vec::Vec;

use hyper;
use hyper_rustls;
use id3;
use log;
use select::document::Document;
use select::predicate::{Class, Name, Predicate};
use tempfile;
use zip;

pub trait Source {
    fn belongs(&self, url: &str) -> bool;
    fn fetch(&self, url: &str, mp3_dir: &Path) -> Result<Album, util::Error>;
    fn description(&self, album: &Album, track: &Track) -> Result<String, util::Error>;
}

const SOURCES: [&dyn Source; 1] = [&Ektoplazm {}];

pub fn fetch(url: &str, mp3_dir: &Path) -> Result<Album, util::Error> {
    for s in &SOURCES {
        if s.belongs(url) {
            return s.fetch(url, mp3_dir);
        }
    }

    Err(util::Error::new(&format!("No source known for {}", url)))
}

pub fn description(album: &Album, track: &Track) -> Result<String, util::Error> {
    for s in &SOURCES {
        if s.belongs(&album.url) {
            return s.description(album, track);
        }
    }

    Err(util::Error::new(&format!(
        "No source known for {}",
        album.url
    )))
}

struct Ektoplazm {}

impl Source for Ektoplazm {
    fn belongs(&self, url: &str) -> bool {
        url.starts_with("https://ektoplazm.com/free-music/")
    }

    fn fetch(&self, url: &str, mp3_dir: &Path) -> Result<Album, util::Error> {
        log::info!("Fetching {}", url);
        let res = download(url)?;
        let (mp3_link, license_link, labels, tags, _) = ektoplazm_parse(res)?;
        let mut res = download(&mp3_link)?;

        let mut tmp = tempfile::tempfile()?;
        copy(&mut res, &mut tmp)?;

        let tmpdir = unpack(tmp, mp3_dir)?;
        let (album_title, album_artist, album_year, tracks) = read_id3(tmpdir.path())?;
        let album = Album {
            url: url.to_string(),

            artist: album_artist,
            title: album_title,
            license: license_link,
            year: album_year,
            labels: labels,
            tags: tags,
            tracks: tracks,
            youtube_id: None,
        };

        let tmpdir = tmpdir.into_path();
        if let Err(e) = std::fs::rename(&tmpdir, album.dirname(mp3_dir)) {
            std::fs::remove_dir_all(tmpdir)?;
            return Err(util::Error::from(e));
        }

        Ok(album)
    }

    fn description(&self, album: &Album, track: &Track) -> Result<String, util::Error> {
        let mut trackno = 99;
        for (n, t) in album.tracks.iter().enumerate() {
            if track == t {
                trackno = n + 1;
                break;
            }
        }

        let mut result = format!("Download the full album from Ektoplazm: {}\n", album.url);

        write!(result, "\nArtist: {}", track.artist)?;
        write!(result, "\nTrack: {}", track.title)?;
        if let Some(y) = album.year {
            write!(result, "\nAlbum: {} ({})", album.title, y)?;
        } else {
            write!(result, "\nAlbum: {}", album.title)?;
        }
        write!(result, "\nTrack number: {:02}", trackno)?;
        if let Some(b) = track.bpm {
            write!(result, "\nBPM: {}", b)?;
        }
        write!(result, "\n")?;
        if !album.tags.is_empty() {
            write!(result, "\nTags: {}", album.tags.join(", "))?;
        }
        if !album.labels.is_empty() {
            write!(result, "\nReleased by: {}", album.labels.join(" & "))?;
        }
        if let Some(l) = &album.license {
            write!(result, "\nLicense: {}", l)?;
        }

        Ok(result)
    }
}

fn track_to_album(album_item: &mut Option<String>, track_item: Option<&str>) {
    if let Some(tr) = track_item {
        if let Some(al) = album_item {
            if tr != al {
                log::warn!(
                    "Album metadata has multiple different values {:?}, {:?}",
                    al,
                    tr
                );
            }
        } else {
            *album_item = Some(tr.to_string());
        }
    }
}

fn required_tag<T>(result: Option<T>, what: &str, path: &Path) -> Result<T, util::Error> {
    result.ok_or(util::Error::new(&format!(
        "{} tag missing in {:?}",
        what, path
    )))
}

fn read_id3(dir: &Path) -> Result<(String, Option<String>, Option<u16>, Vec<Track>), util::Error> {
    let mut tracks: Vec<(u32, Track)> = Vec::new();
    let mut album_artist: Option<String> = None;
    let mut album_title: Option<String> = None;
    let mut album_year: Option<u16> = None;

    for f in std::fs::read_dir(dir)? {
        let f = f?;
        if !f.file_type()?.is_file() {
            continue;
        }
        let path = f.path();

        match path.extension() {
            None => continue,
            Some(ext) => {
                if ext != "mp3" {
                    continue;
                }
            }
        }

        let tag = id3::Tag::read_from_path(&path)?;
        let num = required_tag(tag.track(), "Track number", &path)?;
        tracks.push((
            num,
            Track {
                artist: required_tag(tag.artist(), "Artist", &path)?.to_string(),
                title: required_tag(tag.title(), "Title", &path)?.to_string(),
                bpm: tag
                    .get("TBPM")
                    .and_then(|f| f.content().text())
                    .and_then(|t| t.parse().ok()),
                mp3_file: Some(PathBuf::from(f.file_name())),
                video_file: None,
                youtube_id: None,
            },
        ));

        track_to_album(&mut album_artist, tag.album_artist());
        track_to_album(&mut album_title, tag.album());
        album_year = tag.year().map(|y| y as u16);
    }

    tracks.sort_by_key(|t| t.0);
    let tracks = tracks.into_iter().map(|t| t.1).collect();

    if album_artist == Some("VA".to_string()) {
        album_artist = None;
    }

    match album_title {
        None => Err(util::Error::new("Album artist cannot be determined")),
        Some(t) => Ok((t, album_artist, album_year, tracks)),
    }
}

fn download(url: &str) -> Result<hyper::client::response::Response, util::Error> {
    log::debug!("GET {}", url);
    let client = hyper::Client::with_connector(hyper::net::HttpsConnector::new(
        hyper_rustls::TlsClient::new(),
    ));
    let res = client
        .get(url)
        .header(hyper::header::UserAgent(util::USER_AGENT.to_owned()))
        .send()?;

    //let mut body = Vec::new();
    if res.status != hyper::status::StatusCode::Ok {
        log::error!("Failed to GET {}: {:?}", url, res);
        //let _body_len = res.read_to_end(&mut body)?;
        //log::debug!("{:?}\n{:?}", res, std::str::from_utf8(&body).unwrap());
        return Err(util::Error::new("Failed to fetch URL"));
    }
    log::debug!("Got status {}", res.status);

    Ok(res)
}

fn ektoplazm_parse<T: Read>(
    res: T,
) -> util::Result<(String, Option<String>, Vec<String>, Vec<String>, u32)> {
    let doc = Document::from_read(res)?;

    let mp3_link = match doc
        .find(Class("entry").descendant(Name("a")))
        .filter(|tag| tag.text() == "MP3 Download")
        .filter_map(|tag| tag.attr("href"))
        .next()
    {
        None => return Err(util::Error::new("Failed to find download link")),
        Some(link) => link.to_string(),
    };

    let license_link = doc
        .find(Class("entry").descendant(Name("a")))
        .filter(|tag| {
            tag.attr("href")
                .map_or(false, |target| target.contains("creativecommons"))
        })
        .filter(|tag| {
            let lower = tag.text().to_lowercase();
            lower.contains("license") || lower.contains("licence") || lower.contains("commons")
        })
        .filter_map(|tag| tag.attr("href"))
        .next()
        .map(|x| x.to_string());

    let tags = doc
        .find(Name("h3").descendant(Class("style")).descendant(Name("a")))
        .map(|tag| tag.text())
        .collect();

    let labels = doc
        .find(Name("h3").child(Name("strong")).child(Name("a")))
        .map(|tag| tag.text())
        .collect();

    let tracknum = doc.find(Class("tl").descendant(Class("t"))).count();

    Ok((mp3_link, license_link, labels, tags, tracknum as u32))
}

fn unpack<T: Read + Seek>(res: T, outdir: &Path) -> Result<tempfile::TempDir, util::Error> {
    let mut zip = zip::ZipArchive::new(res)?;

    let tmpdir = tempfile::Builder::new()
        .prefix("0-ektoboat-tmp-")
        .tempdir_in(outdir)?;

    for i in 0..zip.len() {
        let mut zipfile = zip.by_index(i)?;
        let sanitized = zipfile.sanitized_name();
        let basename = match sanitized.file_name() {
            None => {
                log::warn!("Unzip: skipping {:?}", sanitized);
                continue;
            }
            Some(f) => f,
        };

        let mut dest = PathBuf::from(tmpdir.path());
        dest.push(basename);

        log::debug!("Unzip {:?} -> {:?}", basename, dest);
        let mut df = std::fs::File::create(dest)?;
        copy(&mut zipfile, &mut df)?;
    }

    Ok(tmpdir)
}

pub struct EktoplazmScraper {
    next_page: u32,
    urls: VecDeque<String>,
}

impl EktoplazmScraper {
    pub fn from_offset(offset: u32) -> EktoplazmScraper {
        EktoplazmScraper {
            next_page: 1 + offset / 5,
            urls: VecDeque::new(),
        }
    }
}

impl std::iter::Iterator for EktoplazmScraper {
    type Item = Result<String, util::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.urls.is_empty() {
            self.urls = match ektoplazm_scrape_list(self.next_page) {
                Err(e) => return Some(Err(e)),
                Ok(urls) => urls,
            };
            self.next_page += 1;
        }

        let url = self.urls.pop_front()?;
        /*
        let (tracks, zipbytes) = match ektoplazm_scrape_album(&url) {
            Err(e) => return Some(Err(e)),
            Ok((t, z)) => (t, z),
        };
        Some(Ok((url, tracks, zipbytes)))
        */
        Some(Ok(url))
    }
}

fn ektoplazm_scrape_list(offset: u32) -> Result<VecDeque<String>, util::Error> {
    let url = format!("https://ektoplazm.com/section/free-music/page/{}", offset);
    let res = download(&url)?;
    let doc = Document::from_read(res)?;

    let links = doc
        .find(Class("post").child(Name("h1")).child(Name("a")))
        .filter_map(|tag| tag.attr("href"))
        .map(|x| x.to_string())
        .collect();

    Ok(links)
}

/*
fn ektoplazm_scrape_album(url: &str) -> Result<(u32, u32), util::Error> {
    let res = download(url)?;
    let (mp3_link, _, _, _, _tracks) = ektoplazm_parse(res)?;

    let res = download(&mp3_link)?;
    let length = res.headers.get::<hyper::header::ContentLength>().map(|n| n.0 as u32).unwrap_or(0);
    drop(res);

    Ok((tracks, length))
}
*/

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs;
    use std::path::PathBuf;

    fn fixture(fname: &str) -> std::fs::File {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("tests");
        d.push("data");
        d.push(fname);

        fs::File::open(d).unwrap()
    }

    #[test]
    fn parse_ektoplazm() {
        let cases = &[
            (
                "ektoplazm1.html",
                "https://ektoplazm.com/files/Globular%20-%20Entangled%20Everything%20-%202018%20-%20MP3.zip",
                Some("https://creativecommons.org/licenses/by-nc-sa/4.0/"),
                Vec::new(),
                vec!["Downtempo", "Psy Dub"],
                10,
            ),
            (
                "ektoplazm-label.html",
                "https://ektoplazm.com/files/White%20Morph%20-%20Dream%20Catcher%20-%202012%20-%20MP3.zip",
                Some("https://creativecommons.org/licenses/by-nc-sa/3.0/"),
                vec!["3L3Mental Records"],
                vec!["Full-On", "Morning"],
                4,
            ),
            (
                "ektoplazm-va.html",
                "https://ektoplazm.com/files/VA%20-%20Dividing%202%20Worlds%20-%202018%20-%20MP3.zip",
                Some("https://creativecommons.org/licenses/by-nc-sa/4.0/"),
                vec!["Jaira Records"],
                vec!["Techno", "Techtrance", "Zenonesque"],
                9,
            ),
            (
                "ektoplazm-multilabel.html",
                "https://ektoplazm.com/files/Rose%20Red%20Flechette%20-%20The%20Destruction%20Myth%20-%202018%20-%20MP3.zip",
                Some("https://creativecommons.org/licenses/by-nc-sa/4.0/"),
                vec!["Anomalistic Records", "Splatterkore Reck-ords"],
                vec!["Experimental", "Psycore"],
                8,
            ),
            (
                "ektoplazm-license.html",
                "https://ektoplazm.com/files/Ekoplex%20-%20Enter%20The%20Dragon%20EP%20-%202008%20-%20MP3.zip",
                Some("https://creativecommons.org/licenses/by-nc-nd/2.5/ca/"),
                vec!["Ektoplazm"],
                vec!["Full-On"],
                3,
            ),
        ];

        for c in cases {
            let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            d.push("tests");
            d.push("data");
            d.push(c.0);

            let f = fs::File::open(d).unwrap();
            let (mp3, license, labels, tags, tracknum) = ektoplazm_parse(f).unwrap();
            assert_eq!(mp3, c.1.to_string());
            assert_eq!(license, c.2.map(|x| x.to_string()));
            assert_eq!(labels, c.3);
            assert_eq!(tags, c.4);
            assert_eq!(tracknum, c.5);
        }
    }

    #[test]
    fn unpack_id3_ektoplazm() {
        let f = fixture("Risingson - Predestination - 2016 - MP3.zip");
        let expected: HashSet<(String, u64)> = [
            ("00 - Risingson - Predestination.jpg", 2447869),
            ("01 - Risingson - Digital Being.mp3", 12879872),
            ("02 - Risingson - Robosapiens.mp3", 12060672),
            ("03 - Risingson - Predestination.mp3", 12892160),
            ("folder.jpg", 136773),
        ]
        .iter()
        .cloned()
        .map(|(f, s)| (f.to_string(), s))
        .collect();

        let testdir = tempfile::tempdir().unwrap();
        let dir = unpack(f, testdir.path()).unwrap();

        let contents = fs::read_dir(dir.path())
            .unwrap()
            .map(|x| x.unwrap())
            .map(|de| {
                (
                    de.file_name().into_string().unwrap(),
                    de.metadata().unwrap().len(),
                )
            })
            .collect::<HashSet<(_, _)>>();
        assert_eq!(contents, expected);

        let (album_title, album_artist, album_year, tracks) = read_id3(dir.path()).unwrap();
        assert_eq!(album_title, "Predestination".to_string());
        assert_eq!(album_artist, Some("Risingson".to_string()));
        assert_eq!(album_year, Some(2016));
        assert_eq!(
            tracks,
            vec![
                Track {
                    artist: "Risingson".to_string(),
                    title: "Digital Being".to_string(),
                    bpm: Some(88),
                    mp3_file: Some(PathBuf::from("01 - Risingson - Digital Being.mp3")),
                    video_file: None,
                    youtube_id: None,
                },
                Track {
                    artist: "Risingson".to_string(),
                    title: "Robosapiens".to_string(),
                    bpm: Some(97),
                    mp3_file: Some(PathBuf::from("02 - Risingson - Robosapiens.mp3")),
                    video_file: None,
                    youtube_id: None,
                },
                Track {
                    artist: "Risingson".to_string(),
                    title: "Predestination".to_string(),
                    bpm: Some(88),
                    mp3_file: Some(PathBuf::from("03 - Risingson - Predestination.mp3")),
                    video_file: None,
                    youtube_id: None,
                },
            ]
        );
    }

    #[test]
    fn description_ektoplazm() {
        let testcases = [(
            Album {
                url: "https://ektoplazm.com/free-music/asdfasdf".to_string(),

                artist: Some("Risingson".to_string()),
                title: "Forgot".to_string(),
                license: None,
                year: None,
                labels: vec![],
                tags: vec![],
                tracks: vec![],
                youtube_id: None,
            },
            Track {
                artist: "Risingson".to_string(),
                title: "Digital Being".to_string(),
                bpm: Some(88),
                mp3_file: Some(PathBuf::from("01 - Risingson - Digital Being.mp3")),
                video_file: None,
                youtube_id: None,
            },
            "exp",
        )];

        for tc in &testcases {
            println!("{}", description(&tc.0, &tc.1).unwrap());
        }
    }
}
