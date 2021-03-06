use crate::config;
use crate::source;
use crate::store;
use crate::util;
use crate::video;
use crate::youtube;

pub fn run_url(
    config: &config::Config,
    store: &mut store::Store,
    yt: &youtube::YT,
    url: &str,
) -> util::Result<()> {
    let yt_sleep_duration = chrono::Duration::hours(4);
    let blacklist = store.blacklist()?; // maybe don't init this every time in daemon

    log::info!("Processing {}", url);
    let mut album = match store.get_album(url)? {
        None => source::fetch(url, &config.mp3_dir())?,
        Some(album) => {
            if album.has_mp3(&config.mp3_dir()) {
                album
            } else {
                // FIXME deletes yt ids!
                log::warn!("Album has missing audio files, re-fetching");
                return Err(util::Error::new("Refetching overwrites YT ids, FIXME!"));
                // source::fetch(url, &config.mp3_dir())?
            }
        }
    };
    store.save(&album)?;

    if album.license.is_none() {
        return Err(util::Error::new("No license"));
    }
    if blacklist.matches(&album) {
        return Err(util::Error::new("Blacklisted"));
    }

    let album_video_dir = album.dirname(&config.video_dir());
    if !album.has_video(&config.video_dir()) {
        let cover_img = video::find_cover(&album.dirname(&config.mp3_dir()))?;
        let album_mp3_dir = album.dirname(&config.mp3_dir());
        util::mkdir_if_not_exists(&album_video_dir);

        for mut tr in &mut album.tracks {
            let basename = tr.mp3_file.as_ref().ok_or("MP3 file missing")?;
            let mut mp3_file = album_mp3_dir.clone();
            mp3_file.push(basename);

            let mut video_file = album_video_dir.clone();
            let basename = basename.with_extension("avi");
            video_file.push(basename.clone());

            video::convert_file(&mp3_file, &cover_img, &video_file)?;

            tr.video_file = Some(basename);
        }
        store.save(&album)?;
    }
    //TODO: can delete mp3s here

    // generate descriptions first, can't use reference to album inside the for loop
    let descriptions = album
        .tracks
        .iter()
        .map(|t| source::description(&album, t))
        .collect::<Result<Vec<_>, _>>()?;

    for (i, desc) in descriptions.into_iter().enumerate() {
        let tr = album.tracks[i].clone();
        if let Some(yt_id) = &tr.youtube_id {
            log::debug!(
                "Track {} already has youtube id {}",
                tr.title,
                yt_id.as_url()
            );
            continue;
        }

        let mut video_file = album_video_dir.clone();
        video_file.push(tr.video_file.as_ref().ok_or("Video file missing")?);
        let args = youtube::Video {
            title: format!("{} - {}", tr.artist, tr.title),
            description: desc,
            tags: album.tags.clone(),
            filename: video_file,
        };
        let yt_id = util::retry(8, yt_sleep_duration, || yt.upload_video(args.clone()))?;
        album.tracks[i].youtube_id = Some(yt_id);
        store.save(&album)?;
    }
    //TODO: can delete videos here

    if album.youtube_id.is_none() && album.tracks.iter().all(|t| t.youtube_id.is_some()) {
        let args = youtube::Playlist {
            title: youtube::playlist_title(&album.title, &album.artist, &album.year, &album.tags),
            description: String::new(), // the description is not really visible
            tags: album.tags.clone(),
            videos: album
                .tracks
                .iter()
                .map(|t| t.youtube_id.clone().expect("Video ID missing"))
                .collect(),
        };
        let yt_id = util::retry(8, yt_sleep_duration, || yt.create_playlist(args.clone()))?;
        album.youtube_id = Some(yt_id);
        store.save(&album)?;
    }

    log::info!(
        "Success - {} - {}",
        url,
        album
            .youtube_id
            .map_or("(no playlist id)".to_string(), |y| y.to_string())
    );
    Ok(())
}

pub fn daemon(
    config: &config::Config,
    store: &mut store::Store,
    yt: &youtube::YT,
) -> util::Result<()> {
    loop {
        let (act, url) = match store.queue_get()? {
            None => {
                log::error!("No more work!");
                return Ok(());
            }
            Some((act, url)) if act == "url" => (act, url),
            Some((act, _)) => {
                return Err(util::Error::new(&format!("Unknown action {}", act)));
            }
        };
        let res = run_url(config, store, yt, &url);
        let status = match res {
            Err(e) => {
                log::error!("Processing {} failed: {}", url, e);
                e.to_string()
            }
            Ok(()) => "OK".to_string(),
        };
        store.queue_result(act, url, status)?;
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
