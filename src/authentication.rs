use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use librespot_core::authentication::Credentials as RespotCredentials;
use librespot_core::cache::Cache;
use librespot_oauth::OAuthClientBuilder;
use log::{error, info, warn};

use crate::config::{self, Config};
use crate::spotify::Spotify;

const CLIENT_ID_ENV: &str = "NCSPOT_CLIENT_ID";
const SPOTIFY_CLIENT_ID_ENV: &str = "NCSPOT_SPOTIFY_CLIENT_ID";
const REDIRECT_PORT_ENV: &str = "NCSPOT_OAUTH_REDIRECT_PORT";

/// Port used by the local OAuth redirect. Spotify requires registered redirect URIs, so this
/// can't use a random free port.
pub const DEFAULT_REDIRECT_PORT: u16 = 8989;

static OAUTH_SCOPES: &[&str] = &[
    "playlist-modify-private",
    "playlist-modify-public",
    "playlist-read-collaborative",
    "playlist-read-private",
    "streaming",
    "user-follow-modify",
    "user-follow-read",
    "user-library-modify",
    "user-library-read",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "user-read-email",
    "user-read-playback-position",
    "user-read-playback-state",
    "user-read-private",
    "user-read-recently-played",
    "user-top-read",
];

enum ClientUriSuffix {
    Ncspot,
    Spotify,
}

/// The client ID for Web API calls. It can be set in the environment or in the local config.
pub fn client_id(configuration: &Config) -> Result<String, String> {
    client_id_from_env(CLIENT_ID_ENV)
        .or_else(|| configuration.values().client_id.clone())
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .ok_or_else(|| missing_client_id_message(configuration, CLIENT_ID_ENV, "client_id"))
}

/// The client ID for the Spotify session. Defaults to the Web API client ID, so a single Spotify
/// app can be used for both logins.
pub fn spotify_client_id(configuration: &Config) -> Result<String, String> {
    client_id_from_env(SPOTIFY_CLIENT_ID_ENV)
        .or_else(|| configuration.values().spotify_client_id.clone())
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .or_else(|| client_id(configuration).ok())
        .ok_or_else(|| {
            missing_client_id_message(configuration, SPOTIFY_CLIENT_ID_ENV, "spotify_client_id")
        })
}

fn client_id_from_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn missing_client_id_message(configuration: &Config, env_name: &str, config_name: &str) -> String {
    let port = redirect_port(configuration);
    format!(
        "No Spotify client ID configured.\n\
         Set {env_name} or `{config_name}` in config.toml to the client ID of your own Spotify app.\n\
         Add these redirect URIs to the app:\n\
         http://127.0.0.1:{port}/login\n\
         http://127.0.0.1:{port}/ncspot_login\n\
         The client ID is a public OAuth identifier, not a client secret."
    )
}

/// The local port used for OAuth redirects.
pub fn redirect_port(configuration: &Config) -> u16 {
    client_id_from_env(REDIRECT_PORT_ENV)
        .and_then(|value| value.parse::<u16>().ok())
        .or(configuration.values().oauth_redirect_port)
        .unwrap_or(DEFAULT_REDIRECT_PORT)
}

fn get_client_redirect_uri(configuration: &Config, suffix: ClientUriSuffix) -> String {
    let port = redirect_port(configuration);
    match suffix {
        ClientUriSuffix::Ncspot => format!("http://127.0.0.1:{port}/ncspot_login"),
        ClientUriSuffix::Spotify => format!("http://127.0.0.1:{port}/login"),
    }
}

/// Get credentials for use with librespot. This first tries to get cached credentials. If no cached
/// credentials are available it will initiate the OAuth2 login process.
pub fn get_credentials(configuration: &Config) -> Result<RespotCredentials, String> {
    let mut credentials = {
        let cache = Cache::new(Some(config::cache_path("librespot")), None, None, None)
            .expect("Could not create librespot cache");
        let cached_credentials = cache.credentials();
        match cached_credentials {
            Some(c) => {
                info!("Using cached credentials");
                c
            }
            None => {
                info!("Attempting to login via OAuth2");
                credentials_prompt(configuration, None)?
            }
        }
    };

    while let Err(error) = Spotify::test_credentials(configuration, credentials.clone()) {
        let error_msg = format!("{error}");
        credentials = credentials_prompt(configuration, Some(error_msg))?;
    }
    Ok(credentials)
}

fn credentials_prompt(
    configuration: &Config,
    error_message: Option<String>,
) -> Result<RespotCredentials, String> {
    if let Some(message) = error_message {
        eprintln!("Connection error: {message}");
    }

    create_credentials(configuration)
}

pub fn create_credentials(configuration: &Config) -> Result<RespotCredentials, String> {
    let client_id = spotify_client_id(configuration)?;
    println!("To login you need to perform OAuth2 authorization using your web browser\n");
    let client_builder = OAuthClientBuilder::new(
        &client_id,
        &get_client_redirect_uri(configuration, ClientUriSuffix::Spotify),
        OAUTH_SCOPES.to_vec(),
    );
    let oauth_client = client_builder.build().map_err(|e| e.to_string())?;

    oauth_client
        .get_access_token()
        .map(|token| RespotCredentials::with_access_token(token.access_token))
        .map_err(|e| e.to_string())
}

/// Cached or refreshed Web API token. If `allow_login` is false, an expired token without a
/// usable refresh token returns an error instead of starting an interactive login.
pub fn get_rspotify_token(
    configuration: &Config,
    allow_login: bool,
) -> Result<rspotify::Token, String> {
    let client_id = client_id(configuration)?;
    let path = token_path(&client_id);
    let token = if let Ok(token_json) = fs::read_to_string(&path) {
        serde_json::from_str::<rspotify::Token>(&token_json).ok()
    } else {
        None
    };

    if let Some(t) = token {
        if !t.is_expired() {
            return Ok(t);
        }
        if let Ok(token) = refresh_rspotify_token(configuration) {
            return Ok(token);
        }
    }

    if !allow_login {
        return Err(format!(
            "No valid Web API token for this client ID. Restart ncspot to log in again ({}).",
            path.display()
        ));
    }

    let t = create_rspotify_token(configuration)?;
    write_token(&path, &t);
    Ok(t)
}

/// Refresh the cached Web API token using its refresh token, even if the token has not expired
/// yet. This is used after the API answers 401.
pub fn refresh_rspotify_token(configuration: &Config) -> Result<rspotify::Token, String> {
    let client_id = client_id(configuration)?;
    let path = token_path(&client_id);
    let token_json = fs::read_to_string(&path)
        .map_err(|e| format!("No cached Web API token for this client ID: {e}"))?;
    let token: rspotify::Token = serde_json::from_str(&token_json)
        .map_err(|e| format!("Could not parse the cached Web API token: {e}"))?;
    let refresh_token = token
        .refresh_token
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The cached Web API token has no refresh token".to_string())?;

    info!("Attempting to refresh the Web API token");
    let client_builder = OAuthClientBuilder::new(
        &client_id,
        &get_client_redirect_uri(configuration, ClientUriSuffix::Ncspot),
        OAUTH_SCOPES.to_vec(),
    );
    let oauth_client = client_builder.build().map_err(|e| e.to_string())?;
    let new_token = oauth_client
        .refresh_token(refresh_token)
        .map_err(|e| format!("Failed to refresh token: {e}"))?;
    let mapped = map_token(new_token, Some(refresh_token));
    write_token(&path, &mapped);
    Ok(mapped)
}

pub fn create_rspotify_token(configuration: &Config) -> Result<rspotify::Token, String> {
    let client_id = client_id(configuration)?;
    println!(
        "To fully enable Web API features, you need to perform a second OAuth2 authorization\n"
    );
    let client_builder = OAuthClientBuilder::new(
        &client_id,
        &get_client_redirect_uri(configuration, ClientUriSuffix::Ncspot),
        OAUTH_SCOPES.to_vec(),
    );
    let oauth_client = client_builder.build().map_err(|e| e.to_string())?;

    oauth_client
        .get_access_token()
        .map(|token| map_token(token, None))
        .map_err(|e| e.to_string())
}

/// Remove cached Web API tokens for all client IDs.
pub fn remove_rspotify_tokens() {
    let dir = match config::user_cache_directory() {
        Some(dir) => dir,
        None => return,
    };
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            error!("Failed to read the cache directory to remove tokens: {e}");
            return;
        }
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with("rspotify_token")
            && file_name.ends_with(".json")
            && let Err(e) = fs::remove_file(entry.path())
        {
            warn!(
                "Failed to remove token cache {}: {e}",
                entry.path().display()
            );
        }
    }
}

fn token_path(client_id: &str) -> PathBuf {
    config::cache_path(&token_file_name(client_id))
}

fn token_file_name(client_id: &str) -> String {
    let sanitized: String = client_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    if sanitized.is_empty() {
        "rspotify_token_invalid.json".to_string()
    } else {
        format!("rspotify_token_{sanitized}.json")
    }
}

/// Write `token` to `path`, logging (rather than silently discarding) any failure, and
/// restricting file permissions since the file holds a long-lived credential.
fn write_token(path: &Path, token: &rspotify::Token) {
    let json = match serde_json::to_string_pretty(token) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to serialize rspotify token: {e}");
            return;
        }
    };

    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
    };
    #[cfg(not(unix))]
    let file = fs::File::create(path);

    let mut file = match file {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open rspotify token cache: {e}");
            return;
        }
    };
    if let Err(e) = file.write_all(json.as_bytes()) {
        error!("Failed to write rspotify token cache: {e}");
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
            warn!("Failed to restrict permissions on rspotify token cache: {e}");
        }
    }
}

/// Map an OAuth token obtained from librespot into the `rspotify::Token` type used for Web
/// API calls.
///
/// Spotify's refresh-token responses may omit `refresh_token` entirely, in which case the
/// previously issued refresh token must be reused (it has not been rotated or invalidated).
/// `librespot_oauth` maps that omission to an empty string, so `fallback_refresh_token` is
/// substituted whenever the token returned by librespot is empty, to avoid ever persisting an
/// unusable refresh token.
fn map_token(
    token: librespot_oauth::OAuthToken,
    fallback_refresh_token: Option<&str>,
) -> rspotify::Token {
    let duration = if token.expires_at > std::time::Instant::now() {
        token.expires_at.duration_since(std::time::Instant::now())
    } else {
        std::time::Duration::from_secs(0)
    };
    let expires_in = chrono::Duration::from_std(duration).unwrap_or(chrono::Duration::seconds(0));

    let refresh_token = if token.refresh_token.is_empty() {
        fallback_refresh_token.map(str::to_string)
    } else {
        Some(token.refresh_token)
    };

    rspotify::Token {
        access_token: token.access_token,
        expires_in,
        scopes: std::collections::HashSet::new(),
        expires_at: Some(chrono::Utc::now() + expires_in),
        refresh_token,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn oauth_token(refresh_token: &str) -> librespot_oauth::OAuthToken {
        librespot_oauth::OAuthToken {
            access_token: "access".to_string(),
            refresh_token: refresh_token.to_string(),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(3600),
            token_type: "Bearer".to_string(),
            scopes: Vec::new(),
        }
    }

    #[test]
    fn map_token_keeps_new_refresh_token_when_present() {
        let mapped = map_token(oauth_token("new-refresh-token"), Some("old-refresh-token"));
        assert_eq!(mapped.refresh_token, Some("new-refresh-token".to_string()));
    }

    #[test]
    fn map_token_falls_back_to_previous_refresh_token_when_omitted() {
        let mapped = map_token(oauth_token(""), Some("old-refresh-token"));
        assert_eq!(mapped.refresh_token, Some("old-refresh-token".to_string()));
    }

    #[test]
    fn map_token_yields_no_refresh_token_when_omitted_without_fallback() {
        let mapped = map_token(oauth_token(""), None);
        assert_eq!(mapped.refresh_token, None);
    }

    #[test]
    fn token_cache_is_per_client_and_cannot_escape_the_cache_directory() {
        assert_eq!(
            token_file_name("../my-client-id"),
            "rspotify_token_myclientid.json"
        );
        assert_ne!(token_file_name("client-one"), token_file_name("client-two"));
    }
}
