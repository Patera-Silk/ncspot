use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use cursive::Cursive;
use cursive::view::ViewWrapper;

use crate::command::Command;
use crate::commands::CommandResult;
use crate::library::Library;
use crate::model::album::Album;
use crate::model::artist::Artist;
use crate::model::track::Track;
use crate::queue::Queue;
use crate::traits::ViewExt;
use crate::ui::listview::ListView;
use crate::ui::tabbedview::TabbedView;

pub struct AlbumView {
    album: Album,
    tracks: Arc<RwLock<Vec<Track>>>,
    loading: Arc<AtomicBool>,
    tabs: TabbedView,
}

impl AlbumView {
    pub fn new(queue: Arc<Queue>, library: Arc<Library>, album: &Album) -> Self {
        let album = album.clone();
        let tracks = Arc::new(RwLock::new(album.tracks.clone().unwrap_or_default()));
        let loading = Arc::new(AtomicBool::new(!album.is_loaded()));

        let artists = album
            .artist_ids
            .iter()
            .zip(album.artists.iter())
            .map(|(id, name)| Artist::new(id.clone(), name.clone()))
            .collect();

        let mut tabs = TabbedView::new();
        tabs.add_tab(
            "Tracks",
            ListView::new(tracks.clone(), queue.clone(), library.clone()),
        );
        tabs.add_tab(
            "Artists",
            ListView::new(
                Arc::new(RwLock::new(artists)),
                queue.clone(),
                library.clone(),
            ),
        );

        if !album.is_loaded() {
            let mut album = album.clone();
            let tracks = tracks.clone();
            let loading = loading.clone();
            let spotify = queue.get_spotify();
            let library = library.clone();
            thread::spawn(move || {
                album.load_all_tracks(spotify);
                loading.store(false, Ordering::Relaxed);
                if let Some(loaded) = &album.tracks {
                    *tracks.write().unwrap() = loaded.clone();
                    library.album_update(&album);
                    library.trigger_redraw();
                }
            });
        }

        Self {
            album,
            tracks,
            loading,
            tabs,
        }
    }
}

impl ViewWrapper for AlbumView {
    wrap_impl!(self.tabs: TabbedView);
}

impl ViewExt for AlbumView {
    fn title(&self) -> String {
        format!("{} ({})", self.album.title, self.album.year)
    }

    fn title_sub(&self) -> String {
        let tracks = self.tracks.read().unwrap();
        if self.loading.load(Ordering::Relaxed) {
            return "Loading tracks…".to_string();
        }
        let duration_secs: u64 = tracks.iter().map(|t| t.duration as u64 / 1000).sum();
        let duration = std::time::Duration::from_secs(duration_secs);
        let duration_str = crate::utils::format_duration(&duration);
        format!("{} tracks, {}", tracks.len(), duration_str)
    }

    fn on_command(&mut self, s: &mut Cursive, cmd: &Command) -> Result<CommandResult, String> {
        self.tabs.on_command(s, cmd)
    }
}
