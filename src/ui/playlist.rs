use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use cursive::Cursive;
use cursive::view::ViewWrapper;

use crate::command::Command;
use crate::commands::CommandResult;
use crate::library::Library;
use crate::model::playable::Playable;
use crate::model::playlist::Playlist;
use crate::queue::Queue;
use crate::spotify::Spotify;

use crate::traits::ViewExt;
use crate::ui::listview::ListView;

pub struct PlaylistView {
    playlist: Playlist,
    list: ListView<Playable>,
    tracks: Arc<RwLock<Vec<Playable>>>,
    loading: Arc<AtomicBool>,
    spotify: Spotify,
    library: Arc<Library>,
    queue: Arc<Queue>,
}

impl PlaylistView {
    pub fn new(queue: Arc<Queue>, library: Arc<Library>, playlist: &Playlist) -> Self {
        let mut playlist = playlist.clone();
        let tracks = Arc::new(RwLock::new(playlist.tracks.clone().unwrap_or_default()));
        let loading = Arc::new(AtomicBool::new(!playlist.is_loaded()));

        if playlist.is_loaded()
            && let Some(order) = library.cfg.state().playlist_orders.get(&playlist.id)
        {
            playlist.sort(&order.key, &order.direction);
            *tracks.write().unwrap() = playlist.tracks.clone().unwrap_or_default();
        }

        let spotify = queue.get_spotify();
        let list = ListView::new(tracks.clone(), queue.clone(), library.clone());

        if !playlist.is_loaded() {
            let mut playlist = playlist.clone();
            let tracks = tracks.clone();
            let spotify = spotify.clone();
            let library = library.clone();
            let order = library
                .cfg
                .state()
                .playlist_orders
                .get(&playlist.id)
                .cloned();
            thread::spawn(move || {
                playlist.load_tracks(&spotify);
                if !playlist.is_loaded() {
                    return;
                }
                if let Some(order) = order {
                    playlist.sort(&order.key, &order.direction);
                }
                *tracks.write().unwrap() = playlist.tracks.clone().unwrap_or_default();
                library.playlist_update(&playlist);
                library.trigger_redraw();
            });
        }

        Self {
            playlist,
            list,
            tracks,
            loading,
            spotify,
            library,
            queue,
        }
    }
}

impl ViewWrapper for PlaylistView {
    wrap_impl!(self.list: ListView<Playable>);
}

impl ViewExt for PlaylistView {
    fn title(&self) -> String {
        self.playlist.name.clone()
    }

    fn title_sub(&self) -> String {
        let tracks = self.tracks.read().unwrap();
        if self.loading.load(Ordering::Relaxed) {
            return "Loading tracks…".to_string();
        }
        let duration_secs = tracks.iter().map(|p| p.duration() as u64 / 1000).sum();
        let duration = std::time::Duration::from_secs(duration_secs);
        format!(
            "{} tracks, {}",
            tracks.len(),
            crate::utils::format_duration(&duration)
        )
    }

    fn on_command(&mut self, s: &mut Cursive, cmd: &Command) -> Result<CommandResult, String> {
        if let Command::Delete = cmd {
            let pos = self.list.get_selected_index();
            self.playlist.tracks = Some(self.tracks.read().unwrap().clone());

            return if self
                .playlist
                .delete_track(pos, self.spotify.clone(), &self.library)
            {
                self.list.remove(pos);
                Ok(CommandResult::Consumed(None))
            } else {
                Err("Could not delete track.".to_string())
            };
        }

        if let Command::Sort(key, direction) = cmd {
            self.library.cfg.with_state_mut(|state| {
                let order = crate::config::SortingOrder {
                    key: key.clone(),
                    direction: direction.clone(),
                };
                state
                    .playlist_orders
                    .insert(self.playlist.id.clone(), order);
            });

            {
                let mut tracks = self.tracks.write().unwrap();
                self.playlist.tracks = Some(tracks.clone());
                self.playlist.sort(key, direction);
                *tracks = self.playlist.tracks.clone().unwrap_or_default();
            }
            self.list = ListView::new(
                self.tracks.clone(),
                self.queue.clone(),
                self.library.clone(),
            );
            return Ok(CommandResult::Consumed(None));
        }

        self.list.on_command(s, cmd)
    }
}
