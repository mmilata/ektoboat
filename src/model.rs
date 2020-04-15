use crate::util;
use crate::youtube;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube_id: Option<youtube::PlaylistID>,
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

    fn has_files(&self, base_dir: &Path, basenames: Vec<&PathBuf>) -> bool {
        let mut f = self.dirname(base_dir);
        for bn in basenames {
            f.push(bn);
            if !f.is_file() {
                return false;
            }
            assert_eq!(f.pop(), true);
        }
        true
    }

    pub fn has_mp3(&self, base_dir: &Path) -> bool {
        let basenames: Option<Vec<_>> = self.tracks.iter().map(|x| x.mp3_file.as_ref()).collect();
        match basenames {
            None => false,
            Some(bn) => self.has_files(base_dir, bn),
        }
    }

    pub fn has_video(&self, base_dir: &Path) -> bool {
        let basenames: Option<Vec<_>> = self.tracks.iter().map(|x| x.video_file.as_ref()).collect();
        match basenames {
            None => false,
            Some(bn) => self.has_files(base_dir, bn),
        }
    }

    pub fn print(&self) {
        let nf = "(none found)".to_string();
        println!(
            "Artist:  {}",
            self.artist.as_ref().unwrap_or(&"VA".to_string())
        );
        println!("Title:   {}", self.title);
        println!(
            "Year:    {}",
            self.year.map(|n| n.to_string()).unwrap_or(nf.clone())
        );
        println!("License: {}", self.license.as_ref().unwrap_or(&nf));
        println!(
            "Label:   {}",
            if self.labels.is_empty() {
                nf.clone()
            } else {
                self.labels.join(", ")
            }
        );
        println!(
            "Tags:    {}",
            if self.tags.is_empty() {
                nf.clone()
            } else {
                self.tags.join(", ")
            }
        );
        println!(
            "YT:      {}",
            self.youtube_id
                .as_ref()
                .map(|i| i.as_url())
                .unwrap_or(nf.clone())
        );
        println!("Tracks:");
        for (i, t) in self.tracks.iter().enumerate() {
            let tnum = i + 1;
            println!("  {:02} - {} - {}", tnum, t.artist, t.title);
            if let Some(b) = t.bpm {
                println!("       BPM:   {}", b);
            }
            if let Some(f) = &t.mp3_file {
                println!(
                    "       MP3:   {}/{}",
                    self.dirname(&PathBuf::from("mp3"))
                        .into_os_string()
                        .to_string_lossy(),
                    f.clone().into_os_string().to_string_lossy()
                );
            }
            if let Some(f) = &t.video_file {
                println!(
                    "       Video: {}/{}",
                    self.dirname(&PathBuf::from("video"))
                        .into_os_string()
                        .to_string_lossy(),
                    f.clone().into_os_string().to_string_lossy()
                );
            }
            println!(
                "       YT:    {}",
                t.youtube_id
                    .as_ref()
                    .map(|i| i.as_url())
                    .unwrap_or(nf.clone())
            );
        }
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

    // relative to video_subdir
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_file: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub youtube_id: Option<youtube::VideoID>,
}

pub struct Blacklist {
    artists: Vec<Regex>,
    labels: Vec<Regex>,
}

impl Blacklist {
    pub fn new<T>(artists: T, labels: T) -> Result<Blacklist, util::Error>
    where
        T: IntoIterator,
        T::Item: AsRef<str>,
    {
        let mut res = Blacklist {
            artists: vec![],
            labels: vec![],
        };
        let pat = |s: T::Item| format!(r"(?i)^{}$", s.as_ref());

        for s in artists {
            res.artists.push(Regex::new(&pat(s))?);
        }
        for s in labels {
            res.labels.push(Regex::new(&pat(s))?);
        }
        Ok(res)
    }

    pub fn matches(&self, album: &Album) -> bool {
        if self
            .labels
            .iter()
            .any(|r| album.labels.iter().any(|l| r.is_match(l)))
        {
            return true;
        }

        if self
            .artists
            .iter()
            .any(|r| album.tracks.iter().any(|t| r.is_match(&t.artist)))
        {
            return true;
        }

        return false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blacklist() {
        let black_artists = vec!["donald duck", "agh[0o]ri tantrik"];
        let black_labels = vec!["sony", "sonic tantra .*", "supraphon"];
        let blacklist = Blacklist::new(black_artists, black_labels).unwrap();

        let album = Album {
            url: "https://ektoplazm.com/free-music/haltya-japan-anime-punk-sessions-ep".to_string(),
            artist: Some("Haltya".to_string()),
            title: "Japan Anime Punk Sessions EP".to_string(),
            license: Some("https://creativecommons.org/licenses/by-nc-nd/3.0/".to_string()),
            year: Some(2012),
            labels: vec!["Not Really".to_string()],
            tags: vec!["Experimental".to_string(), "Suomi".to_string()],
            tracks: vec![
                Track {
                    artist: "Haltya".to_string(),
                    title: "Övertüre".to_string(),
                    bpm: None,
                    mp3_file: None,
                    video_file: None,
                    youtube_id: None,
                },
                Track {
                    artist: "Haltya".to_string(),
                    title: "Hiro In The Sky With Diamonds".to_string(),
                    bpm: None,
                    mp3_file: None,
                    video_file: None,
                    youtube_id: None,
                },
            ],
            youtube_id: None,
        };
        assert!(!blacklist.matches(&album));

        let album = Album {
            url: "https://ektoplazm.com/free-music/sonic-shamans-vol-ii".to_string(),
            artist: Some("VA".to_string()),
            title: "Sonic Shamans Vol. II".to_string(),
            license: Some("https://creativecommons.org/licenses/by-nc-nd/4.0/".to_string()),
            year: Some(2015),
            labels: vec!["Sonic Tantra Records".to_string()],
            tags: vec!["Darkpsy".to_string(), "Psycore".to_string()],
            tracks: vec![
                Track {
                    artist: "Overdream".to_string(),
                    title: "Skip Sick!".to_string(),
                    bpm: Some(150),
                    mp3_file: None,
                    video_file: None,
                    youtube_id: None,
                },
                Track {
                    artist: "Goch".to_string(),
                    title: "Falling Stone".to_string(),
                    bpm: Some(150),
                    mp3_file: None,
                    video_file: None,
                    youtube_id: None,
                },
            ],
            youtube_id: None,
        };
        assert!(blacklist.matches(&album));

        let album = Album {
            url: "https://ektoplazm.com/free-music/high-tech-mechanica".to_string(),
            artist: Some("VA".to_string()),
            title: "High Tech Mechanica".to_string(),
            license: Some("https://creativecommons.org/licenses/by-nc-sa/3.0/".to_string()),
            year: Some(2011),
            labels: vec!["Cosmic Crew Records".to_string()],
            tags: vec!["Hi-Tech".to_string(), "Psycore".to_string()],
            tracks: vec![
                Track {
                    artist: "Murukhan".to_string(),
                    title: "General Dynamicz".to_string(),
                    bpm: Some(175),
                    mp3_file: None,
                    video_file: None,
                    youtube_id: None,
                },
                Track {
                    artist: "Aghori Tantrik".to_string(),
                    title: "Ending Lunatic Behaviour".to_string(),
                    bpm: Some(165),
                    mp3_file: None,
                    video_file: None,
                    youtube_id: None,
                },
            ],
            youtube_id: None,
        };
        assert!(blacklist.matches(&album));
    }
}
