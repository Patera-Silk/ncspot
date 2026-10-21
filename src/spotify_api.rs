use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::application::ASYNC_RUNTIME;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use log::{debug, error, info};
use rspotify::http::HttpError;
use rspotify::model::{
    AlbumId, AlbumType, ArtistId, CursorBasedPage, EpisodeId, FullAlbum, FullArtist, FullEpisode,
    FullPlaylist, FullShow, FullTrack, ItemPositions, LibraryId, Market, Page, PlayableId,
    PlaylistId, PlaylistResult, PrivateUser, Recommendations, SavedAlbum, SavedTrack, SearchResult,
    SearchType, Show, ShowId, SimplifiedTrack, TrackId, UserId,
};
use rspotify::{AuthCodeSpotify, ClientError, ClientResult, prelude::*};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::config::Config as AppConfig;
use crate::model::album::Album;
use crate::model::artist::Artist;
use crate::model::category::Category;
use crate::model::episode::Episode;
use crate::model::playable::Playable;
use crate::model::playlist::Playlist;
use crate::model::track::Track;
use crate::spotify_worker::WorkerCommand;
use crate::ui::pagination::{ApiPage, ApiResult};

/// Maximum number of times an API call is retried after being rate limited.
const MAX_RATE_LIMIT_RETRIES: u32 = 5;
/// Waiting time before the first retry when the API doesn't say how long to wait.
const RATE_LIMIT_BASE_DELAY: Duration = Duration::from_secs(1);
/// Upper bound for a single wait, so a huge `Retry-After` can't freeze the caller for hours.
const RATE_LIMIT_MAX_DELAY: Duration = Duration::from_secs(60);
/// Spotify development-mode apps reject search page sizes above 10.
const SEARCH_PAGE_LIMIT: u32 = 10;
/// Minimum time between failed token refreshes.
const TOKEN_REFRESH_BACKOFF: Duration = Duration::from_secs(60);

/// Time to wait before retrying a rate limited call.
///
/// Uses the `Retry-After` value (in seconds) when present, otherwise an exponential backoff based
/// on the number of `retries` already made. The result never exceeds [`RATE_LIMIT_MAX_DELAY`].
fn rate_limit_delay(retry_after: Option<u64>, retries: u32) -> Duration {
    let delay = match retry_after {
        Some(seconds) => Duration::from_secs(seconds),
        None => RATE_LIMIT_BASE_DELAY.saturating_mul(2u32.saturating_pow(retries)),
    };
    delay.min(RATE_LIMIT_MAX_DELAY)
}

#[derive(Clone)]
pub struct WebApi {
    /// Rspotify web API.
    api: AuthCodeSpotify,
    /// The username of the logged in user.
    user: Option<String>,
    /// Sender of the mpsc channel to the [Spotify](crate::spotify::Spotify) worker thread.
    worker_channel: Arc<RwLock<Option<mpsc::UnboundedSender<WorkerCommand>>>>,
    /// Time at which the token expires.
    token_expiration: Arc<RwLock<DateTime<Utc>>>,
    /// Instant before which no request should be sent, because the API rate limited us. Shared
    /// by all clones so concurrent calls don't keep hitting the limit while it is in effect.
    rate_limited_until: Arc<Mutex<Option<Instant>>>,
    /// The local configuration, needed to refresh the Web API token.
    cfg: Arc<AppConfig>,
    /// Held while refreshing, so refreshes don't overlap.
    refresh_lock: Arc<Mutex<()>>,
    /// Don't retry a failed token refresh before this instant.
    refresh_failed_until: Arc<Mutex<Option<Instant>>>,
}

impl WebApi {
    pub fn new(cfg: Arc<AppConfig>) -> Self {
        let client_id = crate::authentication::client_id(&cfg).unwrap_or_default();
        let config = rspotify::Config {
            token_refreshing: false,
            ..Default::default()
        };
        let api = AuthCodeSpotify::with_config(
            rspotify::Credentials::new(&client_id, ""),
            rspotify::OAuth::default(),
            config,
        );
        let token_expiration = Arc::new(RwLock::new(Utc::now()));
        // Application::new has already ensured that a token exists. Load it here so that
        // Spotify::new does not have to refresh a still-valid token during startup.
        if let Ok(token) = crate::authentication::get_rspotify_token(&cfg, false) {
            let expires_at = token
                .expires_at
                .unwrap_or_else(|| Utc::now() + ChronoDuration::hours(1));
            *api.token.lock().unwrap() = Some(token);
            *token_expiration.write().unwrap() = expires_at;
        }
        Self {
            api,
            user: None,
            worker_channel: Arc::new(RwLock::new(None)),
            token_expiration,
            rate_limited_until: Arc::new(Mutex::new(None)),
            cfg,
            refresh_lock: Arc::new(Mutex::new(())),
            refresh_failed_until: Arc::new(Mutex::new(None)),
        }
    }

    /// The username of the logged in user.
    pub fn user(&self) -> Option<String> {
        self.user.clone()
    }

    /// Set the username for use with the API.
    pub fn set_user(&mut self, user: Option<String>) {
        self.user = user;
    }

    /// Set the sending end of the channel to the worker thread, managed by
    /// [Spotify](crate::spotify::Spotify).
    pub(crate) fn set_worker_channel(
        &mut self,
        channel: Arc<RwLock<Option<mpsc::UnboundedSender<WorkerCommand>>>>,
    ) {
        self.worker_channel = channel;
    }

    /// Whether the token expires within 5 minutes.
    fn needs_token_update(&self) -> bool {
        let delta = *self.token_expiration.read().unwrap() - Utc::now();
        delta.num_seconds() <= 60 * 5
    }

    /// Refresh the token without starting an interactive login. Safe to call from API worker
    /// threads. A failed refresh is not retried for a minute.
    fn refresh_token_blocking(&self, force: bool) -> bool {
        let _guard = self.refresh_lock.lock().unwrap();
        if !force && !self.needs_token_update() {
            return true;
        }
        if let Some(until) = *self.refresh_failed_until.lock().unwrap()
            && Instant::now() < until
        {
            return false;
        }

        let result = if force {
            crate::authentication::refresh_rspotify_token(&self.cfg)
        } else {
            crate::authentication::get_rspotify_token(&self.cfg, false)
        };
        match result {
            Ok(token) => {
                let expires_at = token
                    .expires_at
                    .unwrap_or_else(|| Utc::now() + ChronoDuration::hours(1));
                *self.api.token.lock().unwrap() = Some(token);
                *self.token_expiration.write().unwrap() = expires_at;
                *self.refresh_failed_until.lock().unwrap() = None;
                true
            }
            Err(e) => {
                error!("Failed to update Web API token for client ID: {e}");
                *self.refresh_failed_until.lock().unwrap() =
                    Some(Instant::now() + TOKEN_REFRESH_BACKOFF);
                false
            }
        }
    }

    /// Update the authentication token when it expires.
    pub fn update_token(&self) -> Option<JoinHandle<()>> {
        if !self.needs_token_update() {
            return None;
        }
        let delta = *self.token_expiration.read().unwrap() - Utc::now();
        info!("Token will expire in {delta}, renewing");
        let client = self.clone();
        Some(ASYNC_RUNTIME.get().unwrap().spawn_blocking(move || {
            client.refresh_token_blocking(false);
        }))
    }

    /// Block until any rate limit cooldown shared between all API calls has elapsed.
    fn wait_for_rate_limit(&self) {
        loop {
            let deadline = *self.rate_limited_until.lock().unwrap();
            match deadline.and_then(|d| d.checked_duration_since(Instant::now())) {
                Some(remaining) if !remaining.is_zero() => thread::sleep(remaining),
                _ => return,
            }
        }
    }

    /// Make every API call wait at least `delay` before being sent.
    fn start_rate_limit_cooldown(&self, delay: Duration) {
        let mut until = self.rate_limited_until.lock().unwrap();
        let deadline = Instant::now() + delay;
        if until.is_none_or(|current| deadline > current) {
            *until = Some(deadline);
        }
    }

    /// Execute `api_call`, retrying when a rate limit is hit or the token has expired.
    ///
    /// Rate limited calls (HTTP 429) are retried up to [`MAX_RATE_LIMIT_RETRIES`] times, waiting
    /// for the duration requested by the `Retry-After` header, or with an exponential backoff if
    /// the header is missing. The wait applies to all concurrent API calls, not only the one that
    /// was rate limited. A call that hits an expired token (HTTP 401) is retried once after
    /// refreshing the token.
    fn api_with_retry<F, R>(&self, api_call: F) -> Option<R>
    where
        F: Fn(&AuthCodeSpotify) -> ClientResult<R>,
    {
        self.api_with_retry_inner(api_call, false)
    }

    /// Like [`Self::api_with_retry`], but an endpoint that is unavailable to this app (for
    /// example because it was removed in Spotify development mode) returns `None` quietly.
    fn api_with_retry_optional<F, R>(&self, api_call: F) -> Option<R>
    where
        F: Fn(&AuthCodeSpotify) -> ClientResult<R>,
    {
        self.api_with_retry_inner(api_call, true)
    }

    fn api_with_retry_inner<F, R>(&self, api_call: F, allow_forbidden: bool) -> Option<R>
    where
        F: Fn(&AuthCodeSpotify) -> ClientResult<R>,
    {
        let mut rate_limit_retries = 0;
        let mut token_refreshed = false;
        loop {
            self.wait_for_rate_limit();
            match api_call(&self.api) {
                Ok(v) => return Some(v),
                Err(ClientError::Http(error)) => {
                    debug!("http error: {error:?}");
                    let HttpError::StatusCode(response) = error.as_ref() else {
                        return None;
                    };
                    let status = response.status();
                    match status {
                        429 if rate_limit_retries < MAX_RATE_LIMIT_RETRIES => {
                            let retry_after = response
                                .header("Retry-After")
                                .and_then(|v| v.parse::<u64>().ok());
                            let waiting_duration =
                                rate_limit_delay(retry_after, rate_limit_retries);
                            debug!(
                                "rate limit hit. waiting {waiting_duration:?} (retry {}/{MAX_RATE_LIMIT_RETRIES})",
                                rate_limit_retries + 1
                            );
                            self.start_rate_limit_cooldown(waiting_duration);
                            rate_limit_retries += 1;
                        }
                        429 => {
                            error!(
                                "giving up after {MAX_RATE_LIMIT_RETRIES} retries: Spotify API rate limit exceeded"
                            );
                            return None;
                        }
                        401 if !token_refreshed => {
                            debug!("token unauthorized. trying refresh..");
                            if !self.refresh_token_blocking(true) {
                                return None;
                            }
                            token_refreshed = true;
                        }
                        403 | 404 if allow_forbidden => {
                            debug!("optional Spotify endpoint is unavailable: {response:?}");
                            return None;
                        }
                        _ => {
                            error!("unhandled api error: {response:?}");
                            return None;
                        }
                    }
                }
                Err(e) => {
                    error!("unhandled api error: {e}");
                    return None;
                }
            }
        }
    }

    /// Append `tracks` at `position` in the playlist with `playlist_id`.
    pub fn append_tracks(
        &self,
        playlist_id: &str,
        tracks: &[Playable],
        position: Option<u32>,
    ) -> Result<PlaylistResult, ()> {
        self.api_with_retry(|api| {
            let trackids: Vec<PlayableId> = tracks
                .iter()
                .filter_map(|playable| playable.into())
                .collect();
            api.playlist_add_items(
                PlaylistId::from_id(playlist_id).unwrap(),
                trackids.iter().map(|id| id.as_ref()),
                position,
            )
        })
        .ok_or(())
    }

    pub fn delete_tracks(
        &self,
        playlist_id: &str,
        snapshot_id: &str,
        playables: &[Playable],
    ) -> Result<PlaylistResult, ()> {
        self.api_with_retry(move |api| {
            let playable_ids: Vec<PlayableId> = playables
                .iter()
                .filter_map(|playable| playable.into())
                .collect();
            let positions = playables
                .iter()
                .map(|playable| [playable.list_index() as u32])
                .collect::<Vec<_>>();
            let item_pos: Vec<ItemPositions> = playable_ids
                .iter()
                .zip(positions.iter())
                .map(|(id, positions)| ItemPositions {
                    id: id.as_ref(),
                    positions,
                })
                .collect();
            api.playlist_remove_specific_occurrences_of_items(
                PlaylistId::from_id(playlist_id).unwrap(),
                item_pos,
                Some(snapshot_id),
            )
        })
        .ok_or(())
    }

    /// Set the playlist with `id` to contain only `tracks`. If the playlist already contains
    /// tracks, they will be removed.
    pub fn overwrite_playlist(&self, id: &str, tracks: &[Playable]) {
        // create mutable copy for chunking
        let mut tracks: Vec<Playable> = tracks.to_vec();

        // we can only send 100 tracks per request
        let mut remainder = if tracks.len() > 100 {
            Some(tracks.split_off(100))
        } else {
            None
        };

        let replace_items = self.api_with_retry(|api| {
            let playable_ids: Vec<PlayableId> = tracks
                .iter()
                .filter_map(|playable| playable.into())
                .collect();
            api.playlist_replace_items(
                PlaylistId::from_id(id).unwrap(),
                playable_ids.iter().map(|p| p.as_ref()),
            )
        });

        if replace_items.is_some() {
            debug!("saved {} tracks to playlist {}", tracks.len(), id);
            while let Some(ref mut tracks) = remainder.clone() {
                // grab the next set of 100 tracks
                remainder = if tracks.len() > 100 {
                    Some(tracks.split_off(100))
                } else {
                    None
                };

                debug!("adding another {} tracks to playlist", tracks.len());
                if self.append_tracks(id, tracks, None).is_ok() {
                    debug!("{} tracks successfully added", tracks.len());
                } else {
                    error!("error saving tracks to playlists {id}");
                    return;
                }
            }
        } else {
            error!("error saving tracks to playlist {id}");
        }
    }

    /// Delete the playlist with the given `id`.
    pub fn delete_playlist(&self, id: &str) -> Result<(), ()> {
        self.api_with_retry(|api| {
            api.library_remove([LibraryId::Playlist(PlaylistId::from_id(id).unwrap())])
        })
        .ok_or(())
    }

    /// Create a playlist with the given `name`, `public` visibility and `description`. Returns the
    /// id of the newly created playlist.
    pub fn create_playlist(
        &self,
        name: &str,
        public: Option<bool>,
        description: Option<&str>,
    ) -> Result<String, ()> {
        let result = self.api_with_retry(|api| {
            api.user_playlist_create(
                UserId::from_id(self.user.as_ref().unwrap()).unwrap(),
                name,
                public,
                None,
                description,
            )
        });
        result.map(|r| r.id.id().to_string()).ok_or(())
    }

    /// Fetch the album with the given `album_id`.
    pub fn album(&self, album_id: &str) -> Result<FullAlbum, ()> {
        debug!("fetching album {album_id}");
        let aid = AlbumId::from_id(album_id).map_err(|_| ())?;
        self.api_with_retry(|api| api.album(aid.clone(), Some(Market::FromToken)))
            .ok_or(())
    }

    /// Fetch the artist with the given `artist_id`.
    pub fn artist(&self, artist_id: &str) -> Result<FullArtist, ()> {
        let aid = ArtistId::from_id(artist_id).map_err(|_| ())?;
        self.api_with_retry(|api| api.artist(aid.clone())).ok_or(())
    }

    /// Fetch the playlist with the given `playlist_id`.
    pub fn playlist(&self, playlist_id: &str) -> Result<FullPlaylist, ()> {
        let pid = PlaylistId::from_id(playlist_id).map_err(|_| ())?;
        self.api_with_retry(|api| api.playlist(pid.clone(), None, Some(Market::FromToken)))
            .ok_or(())
    }

    /// Fetch the track with the given `track_id`.
    pub fn track(&self, track_id: &str) -> Result<FullTrack, ()> {
        let tid = TrackId::from_id(track_id).map_err(|_| ())?;
        self.api_with_retry(|api| api.track(tid.clone(), Some(Market::FromToken)))
            .ok_or(())
    }

    /// Fetch the show with the given `show_id`.
    pub fn show(&self, show_id: &str) -> Result<FullShow, ()> {
        let sid = ShowId::from_id(show_id).map_err(|_| ())?;
        self.api_with_retry(|api| api.get_a_show(sid.clone(), Some(Market::FromToken)))
            .ok_or(())
    }

    /// Fetch the episode with the given `episode_id`.
    pub fn episode(&self, episode_id: &str) -> Result<FullEpisode, ()> {
        let eid = EpisodeId::from_id(episode_id).map_err(|_| ())?;
        self.api_with_retry(|api| api.get_an_episode(eid.clone(), Some(Market::FromToken)))
            .ok_or(())
    }

    /// Get recommendations based on the seeds provided with `seed_artists`, `seed_genres` and
    /// `seed_tracks`.
    pub fn recommendations(
        &self,
        seed_artists: Option<Vec<&str>>,
        seed_genres: Option<Vec<&str>>,
        seed_tracks: Option<Vec<&str>>,
    ) -> Result<Recommendations, ()> {
        self.api_with_retry_optional(|api| {
            let seed_artistids = seed_artists.as_ref().map(|artistids| {
                artistids
                    .iter()
                    .map(|id| ArtistId::from_id(*id).unwrap())
                    .collect::<Vec<ArtistId>>()
            });
            let seed_trackids = seed_tracks.as_ref().map(|trackids| {
                trackids
                    .iter()
                    .map(|id| TrackId::from_id(*id).unwrap())
                    .collect::<Vec<TrackId>>()
            });
            api.recommendations(
                std::iter::empty(),
                seed_artistids,
                seed_genres.clone(),
                seed_trackids,
                Some(Market::FromToken),
                Some(100),
            )
        })
        .ok_or(())
    }

    /// Search for items of `searchtype` using the provided `query`. Limit the results to `limit`
    /// items with the given `offset` from the start.
    pub fn search(
        &self,
        searchtype: SearchType,
        query: &str,
        limit: u32,
        offset: u32,
    ) -> Result<SearchResult, ()> {
        // Spotify development-mode apps reject larger pages.
        let limit = limit.min(SEARCH_PAGE_LIMIT);
        self.api_with_retry(|api| {
            api.search(
                query,
                searchtype,
                Some(Market::FromToken),
                None,
                Some(limit),
                Some(offset),
            )
        })
        .ok_or(())
    }

    /// Fetch all the current user's playlists.
    pub fn current_user_playlist(&self) -> ApiResult<Playlist> {
        const MAX_LIMIT: u32 = 50;
        let spotify = self.clone();
        let fetch_page = move |offset: u32| {
            debug!("fetching user playlists, offset: {offset}");
            spotify.api_with_retry(|api| {
                match api.current_user_playlists_manual(Some(MAX_LIMIT), Some(offset)) {
                    Ok(page) => Ok(ApiPage {
                        offset: page.offset,
                        total: page.total,
                        items: page.items.iter().map(|sp| sp.into()).collect(),
                    }),
                    Err(e) => Err(e),
                }
            })
        };
        ApiResult::new(MAX_LIMIT, Arc::new(fetch_page))
    }

    /// Get the tracks in the playlist given by `playlist_id`.
    pub fn user_playlist_tracks(&self, playlist_id: &str) -> ApiResult<Playable> {
        const MAX_LIMIT: u32 = 100;
        let spotify = self.clone();
        let playlist_id = playlist_id.to_string();
        let fetch_page = move |offset: u32| {
            debug!("fetching playlist {playlist_id} tracks, offset: {offset}");
            spotify.api_with_retry(|api| {
                match api.playlist_items_manual(
                    PlaylistId::from_id(&playlist_id).unwrap(),
                    None,
                    Some(Market::FromToken),
                    Some(MAX_LIMIT),
                    Some(offset),
                ) {
                    Ok(page) => Ok(ApiPage {
                        offset: page.offset,
                        total: page.total,
                        items: page
                            .items
                            .iter()
                            .filter(|pt| {
                                if let Some(t) = pt.item.as_ref()
                                    && !t.is_unknown()
                                {
                                    true
                                } else {
                                    error!("Could not process item {pt:?}, ignoring");
                                    false
                                }
                            })
                            .enumerate()
                            .flat_map(|(index, pt)| {
                                pt.item.as_ref().map(|t| {
                                    let mut playable: Playable = t.into();
                                    // TODO: set these
                                    playable.set_added_at(pt.added_at);
                                    playable.set_list_index(page.offset as usize + index);
                                    playable
                                })
                            })
                            .collect(),
                    }),
                    Err(e) => Err(e),
                }
            })
        };
        ApiResult::new(MAX_LIMIT, Arc::new(fetch_page))
    }

    /// Fetch all the tracks in the album with the given `album_id`. Limit the results to `limit`
    /// items, with `offset` from the beginning.
    pub fn album_tracks(
        &self,
        album_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Page<SimplifiedTrack>, ()> {
        debug!("fetching album tracks {album_id}");
        self.api_with_retry(|api| {
            api.album_track_manual(
                AlbumId::from_id(album_id).unwrap(),
                Some(Market::FromToken),
                Some(limit),
                Some(offset),
            )
        })
        .ok_or(())
    }

    /// Fetch all the albums of the given `artist_id`. `album_type` determines which type of albums
    /// to fetch.
    pub fn artist_albums(
        &self,
        artist_id: &str,
        album_type: Option<AlbumType>,
    ) -> ApiResult<Album> {
        const MAX_SIZE: u32 = 50;
        let spotify = self.clone();
        let artist_id = artist_id.to_string();
        let fetch_page = move |offset: u32| {
            debug!("fetching artist {artist_id} albums, offset: {offset}");
            spotify.api_with_retry(|api| {
                match api.artist_albums_manual(
                    ArtistId::from_id(&artist_id).unwrap(),
                    album_type.as_ref().copied(),
                    Some(Market::FromToken),
                    Some(MAX_SIZE),
                    Some(offset),
                ) {
                    Ok(page) => {
                        let mut albums: Vec<Album> =
                            page.items.iter().map(|sa| sa.into()).collect();
                        albums.sort_by(|a, b| b.year.cmp(&a.year));
                        Ok(ApiPage {
                            offset: page.offset,
                            total: page.total,
                            items: albums,
                        })
                    }
                    Err(e) => Err(e),
                }
            })
        };

        ApiResult::new(MAX_SIZE, Arc::new(fetch_page))
    }

    /// Get all the episodes of the show with the given `show_id`.
    pub fn show_episodes(&self, show_id: &str) -> ApiResult<Episode> {
        const MAX_SIZE: u32 = 50;
        let spotify = self.clone();
        let show_id = show_id.to_string();
        let fetch_page = move |offset: u32| {
            debug!("fetching show {} episodes, offset: {}", show_id, offset);
            spotify.api_with_retry(|api| {
                match api.get_shows_episodes_manual(
                    ShowId::from_id(&show_id).unwrap(),
                    Some(Market::FromToken),
                    Some(50),
                    Some(offset),
                ) {
                    Ok(page) => Ok(ApiPage {
                        offset: page.offset,
                        total: page.total,
                        items: page.items.iter().map(|se| se.into()).collect(),
                    }),
                    Err(e) => Err(e),
                }
            })
        };

        ApiResult::new(MAX_SIZE, Arc::new(fetch_page))
    }

    /// Get the user's saved shows.
    pub fn get_saved_shows(&self, offset: u32) -> Result<Page<Show>, ()> {
        self.api_with_retry(|api| api.get_saved_show_manual(Some(50), Some(offset)))
            .ok_or(())
    }

    /// Add the shows with the given `ids` to the user's library.
    pub fn save_shows(&self, ids: &[&str]) -> Result<(), ()> {
        self.api_with_retry(|api| {
            api.library_add(
                ids.iter()
                    .map(|id| LibraryId::Show(ShowId::from_id(*id).unwrap()))
                    .collect::<Vec<LibraryId>>(),
            )
        })
        .ok_or(())
    }

    /// Remove the shows with `ids` from the user's library.
    pub fn unsave_shows(&self, ids: &[&str]) -> Result<(), ()> {
        self.api_with_retry(|api| {
            api.library_remove(
                ids.iter()
                    .map(|id| LibraryId::Show(ShowId::from_id(*id).unwrap()))
                    .collect::<Vec<LibraryId>>(),
            )
        })
        .ok_or(())
    }

    /// Get the user's followed artists. `last` is an artist id. If it is specified, the artists
    /// after the one with this id will be retrieved.
    pub fn current_user_followed_artists(
        &self,
        last: Option<&str>,
    ) -> Result<CursorBasedPage<FullArtist>, ()> {
        self.api_with_retry(|api| api.current_user_followed_artists(last, Some(50)))
            .ok_or(())
    }

    /// Add the logged in user to the followers of the artists with the given `ids`.
    pub fn user_follow_artists(&self, ids: Vec<&str>) -> Result<(), ()> {
        self.api_with_retry(|api| {
            api.library_add(
                ids.iter()
                    .map(|id| LibraryId::Artist(ArtistId::from_id(*id).unwrap()))
                    .collect::<Vec<LibraryId>>(),
            )
        })
        .ok_or(())
    }

    /// Remove the logged in user to the followers of the artists with the given `ids`.
    pub fn user_unfollow_artists(&self, ids: Vec<&str>) -> Result<(), ()> {
        self.api_with_retry(|api| {
            api.library_remove(
                ids.iter()
                    .map(|id| LibraryId::Artist(ArtistId::from_id(*id).unwrap()))
                    .collect::<Vec<LibraryId>>(),
            )
        })
        .ok_or(())
    }

    /// Get the user's saved albums, starting at the given `offset`. The result is paginated.
    pub fn current_user_saved_albums(&self, offset: u32) -> Result<Page<SavedAlbum>, ()> {
        self.api_with_retry(|api| {
            api.current_user_saved_albums_manual(Some(Market::FromToken), Some(50), Some(offset))
        })
        .ok_or(())
    }

    /// Add the albums with the given `ids` to the user's saved albums.
    pub fn current_user_saved_albums_add(&self, ids: Vec<&str>) -> Result<(), ()> {
        self.api_with_retry(|api| {
            api.library_add(
                ids.iter()
                    .map(|id| LibraryId::Album(AlbumId::from_id(*id).unwrap()))
                    .collect::<Vec<LibraryId>>(),
            )
        })
        .ok_or(())
    }

    /// Remove the albums with the given `ids` from the user's saved albums.
    pub fn current_user_saved_albums_delete(&self, ids: Vec<&str>) -> Result<(), ()> {
        self.api_with_retry(|api| {
            api.library_remove(
                ids.iter()
                    .map(|id| LibraryId::Album(AlbumId::from_id(*id).unwrap()))
                    .collect::<Vec<LibraryId>>(),
            )
        })
        .ok_or(())
    }

    /// Get the user's saved tracks, starting at the given `offset`. The result is paginated.
    pub fn current_user_saved_tracks(&self, offset: u32) -> Result<Page<SavedTrack>, ()> {
        self.api_with_retry(|api| {
            api.current_user_saved_tracks_manual(Some(Market::FromToken), Some(50), Some(offset))
        })
        .ok_or(())
    }

    /// Add the tracks with the given `ids` to the user's saved tracks.
    pub fn current_user_saved_tracks_add(&self, ids: Vec<&str>) -> Result<(), ()> {
        self.api_with_retry(|api| {
            api.library_add(
                ids.iter()
                    .map(|id| LibraryId::Track(TrackId::from_id(*id).unwrap()))
                    .collect::<Vec<LibraryId>>(),
            )
        })
        .ok_or(())
    }

    /// Remove the tracks with the given `ids` from the user's saved tracks.
    pub fn current_user_saved_tracks_delete(&self, ids: Vec<&str>) -> Result<(), ()> {
        self.api_with_retry(|api| {
            api.library_remove(
                ids.iter()
                    .map(|id| LibraryId::Track(TrackId::from_id(*id).unwrap()))
                    .collect::<Vec<LibraryId>>(),
            )
        })
        .ok_or(())
    }

    /// Add the logged in user to the followers of the playlist with the given `id`.
    pub fn user_playlist_follow_playlist(&self, id: &str) -> Result<(), ()> {
        self.api_with_retry(|api| {
            api.library_add([LibraryId::Playlist(PlaylistId::from_id(id).unwrap())])
        })
        .ok_or(())
    }

    /// Get the top tracks of the artist with the given `id`.
    pub fn artist_top_tracks(&self, id: &str) -> Result<Vec<Track>, ()> {
        #[allow(deprecated)]
        self.api_with_retry_optional(|api| {
            api.artist_top_tracks(ArtistId::from_id(id).unwrap(), Some(Market::FromToken))
        })
        .map(|ft| ft.iter().map(|t| t.into()).collect())
        .ok_or(())
    }

    /// Get artists related to the artist with the given `id`.
    pub fn artist_related_artists(&self, id: &str) -> Result<Vec<Artist>, ()> {
        #[allow(deprecated)]
        self.api_with_retry_optional(|api| {
            api.artist_related_artists(ArtistId::from_id(id).unwrap())
        })
        .map(|fa| fa.iter().map(|a| a.into()).collect())
        .ok_or(())
    }

    /// Get the available categories.
    pub fn categories(&self) -> ApiResult<Category> {
        const MAX_LIMIT: u32 = 50;
        let spotify = self.clone();
        let fetch_page = move |offset: u32| {
            debug!("fetching categories, offset: {offset}");
            spotify.api_with_retry_optional(|api| {
                #[allow(deprecated)]
                match api.categories_manual(
                    None,
                    Some(Market::FromToken),
                    Some(MAX_LIMIT),
                    Some(offset),
                ) {
                    Ok(page) => Ok(ApiPage {
                        offset: page.offset,
                        total: page.total,
                        items: page.items.iter().map(|cat| cat.into()).collect(),
                    }),
                    Err(e) => Err(e),
                }
            })
        };
        ApiResult::new(MAX_LIMIT, Arc::new(fetch_page))
    }

    /// Get the playlists in the category given by `category_id`.
    pub fn category_playlists(&self, category_id: &str) -> ApiResult<Playlist> {
        const MAX_LIMIT: u32 = 50;
        let spotify = self.clone();
        let category_id = category_id.to_string();
        let fetch_page = move |offset: u32| {
            debug!("fetching category playlists, offset: {offset}");
            spotify.api_with_retry_optional(|api| {
                #[allow(deprecated)]
                match api.category_playlists_manual(
                    &category_id,
                    Some(Market::FromToken),
                    Some(MAX_LIMIT),
                    Some(offset),
                ) {
                    Ok(page) => Ok(ApiPage {
                        offset: page.offset,
                        total: page.total,
                        items: page.items.iter().map(|sp| sp.into()).collect(),
                    }),
                    Err(e) => Err(e),
                }
            })
        };
        ApiResult::new(MAX_LIMIT, Arc::new(fetch_page))
    }

    /// Get details about the logged in user.
    pub fn current_user(&self) -> Result<PrivateUser, ()> {
        self.api_with_retry(|api| api.current_user()).ok_or(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_delay_honours_retry_after() {
        assert_eq!(rate_limit_delay(Some(6), 0), Duration::from_secs(6));
        assert_eq!(rate_limit_delay(Some(6), 4), Duration::from_secs(6));
    }

    #[test]
    fn rate_limit_delay_backs_off_exponentially_without_retry_after() {
        let delays: Vec<_> = (0..5)
            .map(|n| rate_limit_delay(None, n).as_secs())
            .collect();
        assert_eq!(delays, [1, 2, 4, 8, 16]);
    }

    #[test]
    fn rate_limit_delay_is_capped() {
        assert_eq!(rate_limit_delay(Some(86_400), 0), RATE_LIMIT_MAX_DELAY);
        assert_eq!(rate_limit_delay(None, 100), RATE_LIMIT_MAX_DELAY);
    }
}
