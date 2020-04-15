use std::path::{Path, PathBuf};
use std::process;

use crate::util;

use regex::Regex;

pub fn find_cover(dir: &Path) -> Result<PathBuf, util::Error> {
    let entries: Vec<std::fs::DirEntry> = std::fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;

    let mut fnames: Vec<String> = Vec::new();
    for e in entries {
        if e.file_type()?.is_file() {
            fnames.push(e.file_name().to_string_lossy().to_string());
        }
    }

    let mut result = PathBuf::from(dir);
    result.push(find_cover_vec(fnames)?);
    log::debug!("Using {:?} as a cover", result);
    Ok(result)
}

fn find_cover_vec(mut fnames: Vec<String>) -> Result<String, util::Error> {
    lazy_static! {
        static ref PATTERNS: [Regex; 9] = [
            Regex::new(r"(?i)^00.*Image[ ]?1").unwrap(),
            Regex::new(r"(?i)^00").unwrap(),
            Regex::new(r"(?i)^cover[.]...$").unwrap(),
            Regex::new(r"(?i)front[.]...$").unwrap(),
            Regex::new(r"(?i)image 1").unwrap(),
            Regex::new(r"(?i)cover").unwrap(),
            Regex::new(r"(?i)front").unwrap(),
            Regex::new(r"(?i)^folder[.]jpg$").unwrap(),
            Regex::new(r"").unwrap(),
        ];
    }

    fnames.sort();
    let fnames: Vec<String> = fnames
        .into_iter()
        .filter(|f| f.ends_with(".png") || f.ends_with(".jpg"))
        .collect();

    for re in PATTERNS.iter() {
        for f in &fnames {
            if re.is_match(f) {
                return Ok(f.to_string());
            }
        }
    }

    Err(util::Error::new("No cover image found"))
}

fn temp_video_file(final_file: &Path) -> util::Result<PathBuf> {
    let mut res = final_file.to_path_buf();
    let mut fname = String::from(
        res.file_name()
            .ok_or("Bad video file name")?
            .to_string_lossy(),
    );
    // ffmpeg cares about extensions -> add prefix
    fname.insert_str(0, "TEMP ");
    res.pop();
    res.push(fname);

    Ok(res)
}

pub fn convert_file(
    audio_file: &Path,
    image_file: &Path,
    out_file: &Path,
) -> Result<(), util::Error> {
    log::info!("Converting {:?}", audio_file);

    let temp_file = temp_video_file(out_file)?;

    #[rustfmt::skip]
    let output = process::Command::new("ffmpeg")
        .arg("-loglevel").arg("error")
        .arg("-loop").arg("1")
        .arg("-i").arg(image_file)
        .arg("-i").arg(audio_file)
        .arg("-vf").arg("scale=min(800\\,in_w):-1")
        .arg("-r").arg("1")
        .arg("-acodec").arg("copy")
        .arg("-shortest")
        .arg(&temp_file)
        .output()?;

    if !output.status.success() {
        log::error!("ffmpeg failed");
        log::error!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        log::error!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        return Err(util::Error::new("ffmpeg failed"));
    }

    std::fs::rename(temp_file, out_file)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_cover() {
        let testcases = [
            ("cover.jpg", vec!["00 - hi.mp3", "cover.pdf", "cover.jpg"]),
            (
                "00 - Inner Fhorse Chapter II - Image1.jpg",
                vec![
                    "00 - Inner Fhorse Chapter II - Image2.jpg",
                    "01 - Medicinmannen - Not Another Intro Track.mp3",
                    "02 - Dfectv - I Can't Relax.mp3",
                    "03 - Bodhi - Time Particle Diversion.mp3",
                    "04 - Feargasm - Knas Wunderlich.mp3",
                    "05 - Holix - Hypnotic Story.mp3",
                    "06 - Iguana vs NiceJub - Astrobleme.mp3",
                    "07 - Steganografic - Fingerprints Of The Higgs.mp3",
                    "00 - Inner Fhorse Chapter II - Image1.jpg",
                    "08 - Peyoceps - Randomness Of Speech.mp3",
                    "09 - Dar Kapo - Rejekshun.mp3",
                    "ektobot.json",
                    "folder.jpg",
                ],
            ),
            (
                "00 - Grower - Image 1 (Front).jpg",
                vec![
                    "09 - Xylonite - Thermal Expansion.mp3",
                    "08 - ShizoLizer Gin - Sweet Dino.mp3",
                    "07 - Irukanji - Muti Cappa.mp3",
                    "06 - E.R.S. - I Went Irie (The Orient Funk Experience).mp3",
                    "05 - Overdream - Mystique Cabalistique.mp3",
                    "04 - Vonoom - Dirty Dishwater.mp3",
                    "03 - Mahaon - Leaving The Limit (feat. Locus).mp3",
                    "02 - Spectrum Vision - Outsiders.mp3",
                    "01 - Atati - Versions.mp3",
                    "00 - Grower - Image 6 (Back).jpg",
                    "00 - Grower - Image 5 (CD).jpg",
                    "00 - Grower - Image 4 (Inside 2).jpg",
                    "00 - Grower - Image 3 (Inside 1).jpg",
                    "00 - Grower - Image 2 (Full Cover).jpg",
                    "00 - Grower - Image 1 (Front).jpg",
                ],
            ),
            (
                "00 - Escape Into - The Drama.jpg",
                vec![
                    "03 - Escape Into - H.N.I.mp3",
                    "04 - Escape Into - The Ziggurat Of Wondrous Wonder.mp3",
                    "00 - Escape Into - The Drama.jpg",
                    "folder.jpg",
                    "01 - Escape Into - The Professor.mp3",
                    "02 - Escape Into - Come With Me.mp3",
                    "05 - Escape Into - Outro.mp3",
                ],
            ),
            (
                "[DigitalDiamonds008L]_V.A._Compilation_-_Digital_Family.jpg",
                vec![
                    "[DigitalDiamonds008L]_06_Fuzzion_-_Solar_Alic_Remix.mp3",
                    "[DigitalDiamonds008L]_Coverset.pdf",
                    "Digital Diamonds - Advanced Audio Netlabel.URL",
                    "Creative Commons Attribution-Noncommercial-No Derivative Works 2.0 Germany.URL",
                    "[DigitalDiamonds008L]_04_FM_Radio_Gods_-_Atom_Bells_October_Rust_Remix.mp3",
                    "[DigitalDiamonds008L]_09_Digital_IO_-_Carbon_Classic.mp3",
                    "[DigitalDiamonds008L]_08_Thompson_&_Kuhl_-_Heisse_Luft.mp3",
                    "[DigitalDiamonds008L]_07_Dan_Rotor_-_Gemuesemann.mp3",
                    "[DigitalDiamonds008L]_02_Viker_Turrit_-_Interferon.mp3",
                    "[DigitalDiamonds008L]_V.A._Compilation_-_Digital_Family.jpg",
                    "[DigitalDiamonds008L]_05_BitShift_-_Specialist.mp3",
                    "[DigitalDiamonds008L]_V.A._Compilation_-_Digital_Family.txt",
                    "Ektoplazm - Free Music Portal.URL",
                    "[DigitalDiamonds008L]_01_Dan_Rotor_-_Abducted.mp3",
                    "[DigitalDiamonds008L]_03_Kalumet_-_Blaxun.mp3",
                ],
            ),
            (
                "00 - Trollsås - Image 1.png",
                vec![
                    "01 - Spuge H - SpugeStep.mp3",
                    "00 - Trollsås - Image 2.png",
                    "05 - Salakavala - Pigfoot.mp3",
                    "08 - Anima Animus - Thank You Dr. Hofmann.mp3",
                    "07 - Trance-Ingvars - Maxad Finne.mp3",
                    "04 - Bugswap - Stygian.mp3",
                    "09 - Speedhawk vs Riktronik - Stranger Danger.mp3",
                    "03 - Oliveira - Mystik.mp3",
                    "00 - Trollsås - Info.txt",
                    "folder.jpg",
                    "06 - Scum Unit - Electricity, Vibrations & Frequencies.mp3",
                    "00 - Trollsås - Image 1.png",
                    "02 - Nebula Meltdown - Mindgroove.mp3",
                ],
            ),
        ];

        for tc in &testcases {
            assert_eq!(
                tc.0.to_string(),
                find_cover_vec(tc.1.iter().map(|x| x.to_string()).collect()).unwrap()
            );
        }
    }

    #[test]
    fn temp_video() {
        assert_eq!(
            temp_video_file(&PathBuf::from(
                "/var/lib/ektoboat whatevs/videos/01 - foo - bar.avi"
            ))
            .unwrap(),
            PathBuf::from("/var/lib/ektoboat whatevs/videos/TEMP 01 - foo - bar.avi")
        );
    }
}
