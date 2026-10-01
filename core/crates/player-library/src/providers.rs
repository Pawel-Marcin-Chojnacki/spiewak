//! Online discovery is read-only. Explicitly reviewed candidates use the existing reversible editor.
use super::*;
use serde_json::Value;
use std::{
    io::Read,
    sync::{Mutex, OnceLock},
    time::Duration,
};
const JSON_LIMIT: usize = 2 * 1024 * 1024;
const ART_LIMIT: usize = 8 * 1024 * 1024;
const CACHE_LIMIT: i64 = 128 * 1024 * 1024;
const USER_AGENT: &str = "Spiewak/0.1.0 (https://github.com/Pawel-Marcin-Chojnacki/spiewak)";
// This serializes only provider network calls; it never owns the audio or UI library locks.
static NETWORK: OnceLock<Mutex<()>> = OnceLock::new();
#[derive(Clone, Debug, Default)]
pub struct Candidate {
    pub id: i64,
    pub track_id: i64,
    pub flags: u32,
    pub score: u32,
    pub duration: f64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub date: String,
    pub edition: String,
    pub evidence: String,
    pub source: String,
    pub artwork: String,
    pub recording: String,
    pub release: String,
}
#[derive(Default)]
pub struct Discovery {
    pub enabled: bool,
    pub title: String,
    pub artist: String,
    pub message: String,
    pub candidates: Vec<Candidate>,
}
/// Network results are staged into the artwork/editor cache only after the caller obtains the scan gate.
pub struct DiscoveryProposal {
    track: i64,
    candidate: i64,
    values: Vec<MetadataValue>,
    hash: String,
    identity: String,
    provenance: String,
    cover: Option<Vec<u8>>,
}
struct Response {
    status: u16,
    body: Vec<u8>,
    location: Option<String>,
    retry: Option<u64>,
}
trait Fetch {
    fn get(&self, url: &str, limit: usize) -> Result<Response>;
}
struct Online;
impl Fetch for Online {
    fn get(&self, url: &str, limit: usize) -> Result<Response> {
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(15)))
            .timeout_connect(Some(Duration::from_secs(5)))
            .build();
        let agent = ureq::Agent::new_with_config(config);
        let mut response = agent.get(url).header("User-Agent", USER_AGENT).header("Accept", "application/json, image/png, image/jpeg").call()
            .map_err(|_| "Provider connection failed or timed out. Cached results and playback are available.".to_owned())?;
        let status = response.status().as_u16();
        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let retry = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        if response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok())
            .is_some_and(|n| n > limit)
        {
            return Err("Provider response exceeds the download limit.".into());
        }
        let mut body = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(limit as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|_| "Provider response could not be read completely.".to_owned())?;
        if body.len() > limit {
            return Err("Provider response exceeds the download limit.".into());
        }
        Ok(Response {
            status,
            body,
            location,
            retry,
        })
    }
}
pub(super) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch("SAVEPOINT migration;
 ALTER TABLE file_operations ADD COLUMN provenance TEXT NOT NULL DEFAULT '';
 CREATE TABLE provider_settings(id INTEGER PRIMARY KEY CHECK(id=1),enabled INTEGER NOT NULL DEFAULT 0);
 INSERT INTO provider_settings(id) VALUES(1);
 CREATE TABLE provider_http(url TEXT PRIMARY KEY,status INTEGER NOT NULL,body BLOB NOT NULL,fetched INTEGER NOT NULL,expires INTEGER NOT NULL);
 CREATE TABLE provider_gate(host TEXT PRIMARY KEY,next_request INTEGER NOT NULL);
 CREATE TABLE metadata_candidates(id INTEGER PRIMARY KEY AUTOINCREMENT,track_id INTEGER NOT NULL,flags INTEGER NOT NULL,score INTEGER NOT NULL,duration REAL NOT NULL,title TEXT NOT NULL,artist TEXT NOT NULL,album TEXT NOT NULL,date TEXT NOT NULL,edition TEXT NOT NULL,evidence TEXT NOT NULL,source TEXT NOT NULL,artwork TEXT NOT NULL DEFAULT '',recording TEXT NOT NULL,release TEXT NOT NULL,source_hash TEXT NOT NULL,source_identity TEXT NOT NULL,query_title TEXT NOT NULL,query_artist TEXT NOT NULL);
 CREATE INDEX candidate_track ON metadata_candidates(track_id,id);
 PRAGMA user_version=12; RELEASE migration;").map_err(|e|e.to_string())
}
#[cfg(test)]
pub(super) fn drop_migration(db: &Connection) {
    db.execute_batch("ALTER TABLE file_operations DROP COLUMN provenance; DROP TABLE provider_settings; DROP TABLE provider_http; DROP TABLE provider_gate; DROP TABLE metadata_candidates;").unwrap();
}
fn millis() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
fn mbid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}
fn host(url: &str, art: bool) -> Result<String> {
    let uri = url
        .parse::<ureq::http::Uri>()
        .map_err(|_| "Invalid provider URL.")?;
    let name = uri.host().unwrap_or("").to_ascii_lowercase();
    if uri.scheme_str() != Some("https")
        || uri.authority().is_some_and(|a| a.as_str().contains('@'))
        || uri.port_u16().is_some_and(|p| p != 443)
        || !(name == "musicbrainz.org"
            || art
                && (name == "coverartarchive.org"
                    || name == "archive.org"
                    || name.ends_with(".archive.org")))
    {
        return Err("Provider redirect is outside the supported HTTPS services.".into());
    }
    Ok(name)
}
fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| s.len() <= 16384 && !s.contains('\0'))
        .unwrap_or("")
        .to_owned()
}
fn array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
fn artist(v: &Value) -> String {
    array(v, "artist-credit")
        .iter()
        .take(30)
        .map(|c| {
            let name = text(c, "name");
            format!(
                "{}{}",
                if name.is_empty() {
                    text(&c["artist"], "name")
                } else {
                    name
                },
                text(c, "joinphrase")
            )
        })
        .collect::<String>()
}
fn normalized(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}
fn versions(s: &str) -> std::collections::BTreeSet<&'static str> {
    let words = s
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    [
        "live",
        "remix",
        "acoustic",
        "instrumental",
        "cover",
        "remaster",
        "remastered",
        "edit",
        "extended",
        "karaoke",
        "demo",
        "mix",
        "slowed",
        "sped",
        "reverb",
        "mono",
        "stereo",
    ]
    .into_iter()
    .filter(|marker| words.iter().any(|w| w == marker))
    .map(|m| if m == "remastered" { "remaster" } else { m })
    .collect()
}
// Preserve meaningful annotations even when they are not in the small known-version vocabulary.
fn retained_title(provider: &str, sources: &[&str]) -> String {
    let mut title = provider.to_owned();
    for source in sources {
        let cleaned = metadata::clean_download_name(source);
        let mut tail = cleaned.as_str();
        while let Some(start) = tail.find(['(', '[']) {
            let part = &tail[start..];
            let close = if part.starts_with('(') { ')' } else { ']' };
            let Some(end) = part.find(close) else {
                break;
            };
            let annotation = &part[..=end];
            let words = normalized(annotation);
            if !words.is_empty() && !normalized(&title).contains(&words) {
                title.push(' ');
                title.push_str(annotation);
            }
            tail = &part[end + 1..];
        }
    }
    title
}
fn terms(s: &str) -> Result<String> {
    let s = s.trim();
    if s.is_empty() || s.len() > 512 || s.chars().any(char::is_control) {
        return Err(
            "Enter an artist and title of at most 512 UTF-8 bytes, without control characters."
                .into(),
        );
    }
    Ok(s.to_owned())
}
impl Library {
    pub fn discovery_enabled(&self) -> Result<bool> {
        self.db
            .query_row(
                "SELECT enabled FROM provider_settings WHERE id=1",
                [],
                |r| r.get::<_, bool>(0),
            )
            .map_err(|e| e.to_string())
    }
    pub fn set_discovery_enabled(&self, enabled: bool) -> Result<()> {
        self.db
            .execute(
                "UPDATE provider_settings SET enabled=? WHERE id=1",
                [enabled],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn provider_bytes(
        &self,
        url: &str,
        limit: usize,
        art: bool,
        fetch: &impl Fetch,
    ) -> Result<Option<Vec<u8>>> {
        host(url, art)?;
        let _network = NETWORK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .map_err(|_| "Provider queue unavailable.")?;
        if let Some((status, body)) = self
            .db
            .query_row(
                "SELECT status,body FROM provider_http WHERE url=? AND expires>?",
                params![url, millis()],
                |r| Ok((r.get::<_, u16>(0)?, r.get::<_, Vec<u8>>(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?
        {
            if body.len() > limit {
                return Err("Cached provider response is too large.".into());
            }
            return Ok((status != 404).then_some(body));
        }
        if !self.discovery_enabled()? {
            return Err("Online lookup is disabled. Enable it to send artist/title searches and download artwork.".into());
        }
        let initial = host(url, art)?;
        let mut current = url.to_owned();
        for redirect in 0..=5 {
            let name = host(&current, art)?;
            // Reserve request starts durably, including between two running player processes.
            loop {
                let tx = rusqlite::Transaction::new_unchecked(
                    &self.db,
                    rusqlite::TransactionBehavior::Immediate,
                )
                .map_err(|e| e.to_string())?;
                self.db
                    .execute("INSERT OR IGNORE INTO provider_gate VALUES(?,0)", [&name])
                    .map_err(|e| e.to_string())?;
                let next: i64 = self
                    .db
                    .query_row(
                        "SELECT next_request FROM provider_gate WHERE host=?",
                        [&name],
                        |r| r.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                let wait = next - millis();
                if wait > 0 {
                    drop(tx);
                    if wait > 2000 {
                        return Err(format!(
                            "Provider is backing off; retry in {} seconds. Playback continues.",
                            (wait + 999) / 1000
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(wait as u64));
                    continue;
                }
                self.db
                    .execute(
                        "UPDATE provider_gate SET next_request=? WHERE host=?",
                        params![millis() + 1100, name],
                    )
                    .map_err(|e| e.to_string())?;
                tx.commit().map_err(|e| e.to_string())?;
                break;
            }
            if !self.discovery_enabled()? {
                return Err("Online lookup was disabled before this request.".into());
            }
            let reply = match fetch.get(&current, limit) {
                Ok(response) => response,
                Err(e) => {
                    self.provider_backoff(&name, 30)?;
                    return Err(e);
                }
            };
            if reply.body.len() > limit {
                return Err("Provider response exceeds the download limit.".into());
            }
            if [301, 302, 303, 307, 308].contains(&reply.status) {
                if redirect == 5 {
                    return Err("Artwork redirect limit reached.".into());
                }
                let target = reply
                    .location
                    .ok_or("Provider redirect has no destination.")?;
                current = if target.starts_with("//") {
                    format!("https:{target}")
                } else if target.starts_with('/') {
                    format!("https://{name}{target}")
                } else if let Some(t) = target.strip_prefix("http://") {
                    format!("https://{t}")
                } else {
                    target
                };
                if !art && host(&current, false)? != initial {
                    return Err("Unexpected metadata redirect.".into());
                }
                host(&current, art)?;
                continue;
            }
            if reply.status == 429 || reply.status == 503 {
                self.provider_backoff(&name, reply.retry.unwrap_or(60).clamp(30, 86400))?;
                return Err(
                    "Provider is busy or rate-limited. Results were not applied; try again later."
                        .into(),
                );
            }
            if ![200, 404].contains(&reply.status) {
                self.provider_backoff(&name, 30)?;
                return Err(format!(
                    "Provider returned HTTP {}. No music was changed.",
                    reply.status
                ));
            }
            let ttl = if reply.status == 404 {
                3600
            } else if initial == "musicbrainz.org" {
                7 * 86400
            } else {
                86400
            };
            let body = if reply.status == 404 {
                Vec::new()
            } else {
                reply.body
            };
            let tx = rusqlite::Transaction::new_unchecked(
                &self.db,
                rusqlite::TransactionBehavior::Immediate,
            )
            .map_err(|e| e.to_string())?;
            self.db
                .execute(
                    "INSERT OR REPLACE INTO provider_http VALUES(?,?,?,?,?)",
                    params![url, reply.status, body, millis(), millis() + ttl * 1000],
                )
                .map_err(|e| e.to_string())?;
            while self
                .db
                .query_row(
                    "SELECT coalesce(sum(length(body)),0) FROM provider_http",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .map_err(|e| e.to_string())?
                > CACHE_LIMIT
            {
                self.db.execute("DELETE FROM provider_http WHERE url=(SELECT url FROM provider_http ORDER BY fetched LIMIT 1)",[]).map_err(|e|e.to_string())?;
            }
            tx.commit().map_err(|e| e.to_string())?;
            return Ok((reply.status != 404).then_some(body));
        }
        Err("Provider redirects did not finish.".into())
    }
    fn provider_backoff(&self, name: &str, seconds: u64) -> Result<()> {
        self.db
            .execute(
                "UPDATE provider_gate SET next_request=? WHERE host=?",
                params![millis() + seconds as i64 * 1000, name],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn provider_json(&self, url: &str, art: bool, fetch: &impl Fetch) -> Result<Option<Value>> {
        let Some(body) = self.provider_bytes(url, JSON_LIMIT, art, fetch)? else {
            return Ok(None);
        };
        match serde_json::from_slice(&body) {
            Ok(value) => Ok(Some(value)),
            Err(_) => {
                self.db
                    .execute("DELETE FROM provider_http WHERE url=?", [url])
                    .map_err(|e| e.to_string())?;
                self.provider_backoff(&host(url, art)?, 30)?;
                Err("Provider returned malformed metadata. No music was changed.".into())
            }
        }
    }
    fn candidate(&self, id: i64) -> Result<Candidate> {
        self.db.query_row("SELECT id,track_id,flags,score,duration,title,artist,album,date,edition,evidence,source,artwork,recording,release FROM metadata_candidates WHERE id=?",[id],candidate_row).map_err(|_|"This candidate is no longer cached. Search again.".into())
    }
    pub fn discovery(&self, track: i64) -> Result<Discovery> {
        let enabled = self.discovery_enabled()?;
        if track == 0 {
            return Ok(Discovery {
                enabled,
                ..Default::default()
            });
        }
        let meta = self.metadata(track)?;
        let get = |f| {
            meta.values
                .iter()
                .find(|v| v.field == f)
                .map(|v| v.value.clone())
                .unwrap_or_default()
        };
        let mut title = get(MetadataField::Title);
        let mut artist = get(MetadataField::Artist);
        let suggestion = self.cleanup_suggestion(track)?;
        if title.is_empty() {
            title = suggestion
                .iter()
                .find(|v| v.field == MetadataField::Title)
                .map(|v| v.value.clone())
                .unwrap_or_default();
        }
        if artist.is_empty() {
            artist = suggestion
                .iter()
                .find(|v| v.field == MetadataField::Artist)
                .map(|v| v.value.clone())
                .unwrap_or_default();
        }
        if title.is_empty() {
            let stem = Path::new(&meta.original_path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            if let Some((a, t)) = stem.split_once(" - ") {
                if artist.is_empty() {
                    artist = a.to_owned();
                }
                title = t.to_owned();
            } else {
                title = stem;
            }
        }
        let candidates=self.db.prepare("SELECT id,track_id,flags,score,duration,title,artist,album,date,edition,evidence,source,artwork,recording,release FROM metadata_candidates WHERE track_id=? ORDER BY score DESC,id LIMIT 40").map_err(|e|e.to_string())?.query_map([track],candidate_row).map_err(|e|e.to_string())?.collect::<std::result::Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        Ok(Discovery {
            enabled,
            title,
            artist,
            message: "Candidates need review. Search rank is not a probability of identity.".into(),
            candidates,
        })
    }
    pub fn discover_metadata(&self, track: i64, title: &str, artist: &str) -> Result<Discovery> {
        self.discover_with(track, title, artist, &Online)
    }
    fn discover_with(
        &self,
        track: i64,
        title: &str,
        artist_name: &str,
        fetch: &impl Fetch,
    ) -> Result<Discovery> {
        let title = terms(title)?;
        let artist_name = terms(artist_name)?;
        let source = self.selected_tracks(&[track])?.remove(0);
        let (hash, identity) = self.metadata_origin(track)?;
        let local = self.metadata(track)?;
        let local_title = local
            .values
            .iter()
            .find(|v| v.field == MetadataField::Title)
            .map(|v| v.value.as_str())
            .unwrap_or("");
        let stem = Path::new(&source.path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let filename_title = stem
            .split_once(" - ")
            .map(|(_, title)| title)
            .unwrap_or(&stem);
        let source_versions = versions(&format!("{local_title} {filename_title}"));
        let query = format!(
            "recording:{} AND artist:{}",
            quoted(&title),
            quoted(&artist_name)
        );
        let url = format!(
            "https://musicbrainz.org/ws/2/recording/?query={}&limit=10&fmt=json",
            encode(&query)
        );
        let value = self
            .provider_json(&url, false, fetch)?
            .ok_or("Recording search is unavailable.")?;
        let records = value
            .get("recordings")
            .and_then(Value::as_array)
            .ok_or("Provider response has no recording list.")?;
        let mut candidates = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for record in records.iter().take(10) {
            let recording = text(record, "id");
            if !mbid(&recording) {
                continue;
            }
            let rtitle = text(record, "title");
            let rartist = artist(record);
            if rtitle.is_empty() || rartist.is_empty() {
                continue;
            }
            let duration = record["length"].as_u64().unwrap_or(0).min(86400000) as f64 / 1000.0;
            let score = record["score"]
                .as_u64()
                .or_else(|| record["score"].as_str().and_then(|s| s.parse().ok()))
                .unwrap_or(0)
                .min(100) as u32;
            let disambiguation = text(record, "disambiguation");
            let releases = array(record, "releases");
            let choices: Vec<&Value> = if releases.is_empty() {
                vec![&Value::Null]
            } else {
                releases.iter().take(4).collect()
            };
            for release in choices {
                let release_id = text(release, "id");
                if !release_id.is_empty() && !mbid(&release_id) {
                    continue;
                }
                if !seen.insert((recording.clone(), release_id.clone())) {
                    continue;
                }
                let secondary = array(&release["release-group"], "secondary-types")
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");
                let edition = [
                    text(release, "country"),
                    text(release, "status"),
                    text(release, "disambiguation"),
                    disambiguation.clone(),
                    secondary.clone(),
                ]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
                let provider_versions = versions(&format!("{rtitle} {disambiguation} {secondary}"));
                let version_equal =
                    versions(&title) == provider_versions && source_versions == provider_versions;
                let flags = u32::from(normalized(&title) == normalized(&rtitle))
                    | (u32::from(normalized(&artist_name) == normalized(&rartist)) << 1)
                    | (u32::from(
                        source.duration > 0.0
                            && duration > 0.0
                            && (source.duration - duration).abs() <= 3.0,
                    ) << 2)
                    | (u32::from(version_equal) << 3);
                let evidence = format!(
                    "Needs review · title {} · artist {} · duration {} · version {}. MusicBrainz search rank {score}/100 is not identity confidence.",
                    if flags & 1 != 0 { "matches" } else { "differs" },
                    if flags & 2 != 0 { "matches" } else { "differs" },
                    if flags & 4 != 0 {
                        "within 3s"
                    } else {
                        "different/unknown"
                    },
                    if version_equal {
                        "no marker conflict"
                    } else {
                        "CONFLICT"
                    }
                );
                candidates.push(Candidate {
                    track_id: track,
                    flags,
                    score,
                    duration,
                    title: rtitle.clone(),
                    artist: rartist.clone(),
                    album: text(release, "title"),
                    date: text(release, "date"),
                    edition,
                    evidence,
                    source: format!("https://musicbrainz.org/recording/{recording}"),
                    recording: recording.clone(),
                    release: release_id,
                    ..Default::default()
                });
            }
        }
        // No cached candidate is usable after a concurrent source change.
        if self.metadata_origin(track)? != (hash.clone(), identity.clone()) {
            return Err("File changed during lookup. Search again; no music was changed.".into());
        }
        let tx = rusqlite::Transaction::new_unchecked(
            &self.db,
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(|e| e.to_string())?;
        self.db
            .execute("DELETE FROM metadata_candidates WHERE track_id=?", [track])
            .map_err(|e| e.to_string())?;
        for row in candidates {
            self.db.execute("INSERT INTO metadata_candidates(track_id,flags,score,duration,title,artist,album,date,edition,evidence,source,recording,release,source_hash,source_identity,query_title,query_artist) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",params![track,row.flags,row.score,row.duration,row.title,row.artist,row.album,row.date,row.edition,row.evidence,row.source,row.recording,row.release,hash,identity,title,artist_name]).map_err(|e|e.to_string())?;
        }
        self.db.execute("DELETE FROM metadata_candidates WHERE id NOT IN(SELECT id FROM metadata_candidates ORDER BY id DESC LIMIT 5000)",[]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        let mut result = self.discovery(track)?;
        result.title = title;
        result.artist = artist_name;
        if result.candidates.is_empty() {
            result.message = "No recording found. Your file stays unchanged and playable.".into();
        }
        Ok(result)
    }
    fn candidate_origin(&self, id: i64) -> Result<(String, String, String)> {
        self.db.query_row("SELECT source_hash,source_identity,query_title FROM metadata_candidates WHERE id=?",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())
    }
    pub fn discovery_proposal(
        &self,
        id: i64,
        replace_tags: bool,
        replace_cover: bool,
    ) -> Result<DiscoveryProposal> {
        self.proposal_with(id, replace_tags, replace_cover, &Online)
    }
    fn proposal_with(
        &self,
        id: i64,
        replace_tags: bool,
        replace_cover: bool,
        fetch: &impl Fetch,
    ) -> Result<DiscoveryProposal> {
        let row = self.candidate(id)?;
        if row.flags & 8 == 0 {
            return Err("Different version markers. Choose a candidate that preserves Live/Remix/Acoustic/etc.; no music was changed.".into());
        }
        let (hash, identity, query_title) = self.candidate_origin(id)?;
        if self.metadata_origin(row.track_id)? != (hash.clone(), identity.clone()) {
            return Err("File changed since discovery. Search again.".into());
        }
        // A candidate's previous preview image must not imply a cover change in this proposal.
        self.db
            .execute("UPDATE metadata_candidates SET artwork='' WHERE id=?", [id])
            .map_err(|e| e.to_string())?;
        let current = self.metadata(row.track_id)?;
        let existing = |field| {
            current
                .values
                .iter()
                .find(|v| v.field == field)
                .map(|v| v.value.as_str())
                .unwrap_or("")
        };
        let mut values = Vec::new();
        let mut add = |field, value: String| {
            if !value.is_empty()
                && (replace_tags || existing(field).is_empty())
                && existing(field) != value
            {
                values.push(MetadataValue { field, value });
            }
        };
        let source_track = self.selected_tracks(&[row.track_id])?.remove(0);
        let stem = Path::new(&source_track.path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let filename_title = stem
            .split_once(" - ")
            .map(|(_, title)| title)
            .unwrap_or(&stem);
        add(
            MetadataField::Title,
            retained_title(
                &row.title,
                &[existing(MetadataField::Title), filename_title, &query_title],
            ),
        );
        add(MetadataField::Artist, row.artist.clone());
        if !row.release.is_empty() {
            let url = format!(
                "https://musicbrainz.org/ws/2/release/{}?inc=recordings%2Bartist-credits%2Bgenres&fmt=json",
                row.release
            );
            let release = self
                .provider_json(&url, false, fetch)?
                .ok_or("The selected release is unavailable.")?;
            if text(&release, "id") != row.release {
                return Err("Provider returned a different release; no music was changed.".into());
            }
            let mut matches = Vec::new();
            for medium in array(&release, "media") {
                for track in array(medium, "tracks") {
                    if text(&track["recording"], "id") == row.recording {
                        matches.push((medium, track));
                    }
                }
            }
            if matches.is_empty() {
                return Err("This release no longer contains the selected recording. Choose another edition.".into());
            }
            add(MetadataField::Album, text(&release, "title"));
            add(MetadataField::AlbumArtist, artist(&release));
            let date = text(&release, "date");
            if !date.is_empty() {
                add(MetadataField::Date, date);
            }
            if matches.len() == 1 {
                let (medium, track) = matches[0];
                for (field, value) in [
                    (MetadataField::TrackNumber, track["position"].as_u64()),
                    (MetadataField::TrackTotal, medium["track-count"].as_u64()),
                    (MetadataField::DiscNumber, medium["position"].as_u64()),
                    (
                        MetadataField::DiscTotal,
                        Some(array(&release, "media").len() as u64),
                    ),
                ] {
                    if let Some(value) = value
                        && value > 0
                        && value <= 9999
                    {
                        add(field, value.to_string());
                    }
                }
            }
        }
        let mut warning = String::new();
        let mut cover = None;
        let local_art = self.selected_tracks(&[row.track_id])?.remove(0).artwork;
        if !row.release.is_empty()
            && (replace_cover
                || local_art.is_empty()
                    && (existing(MetadataField::Artwork).is_empty()
                        || existing(MetadataField::Artwork) == "0 embedded picture(s)"))
        {
            match self.download_cover(&row, fetch) {
                Ok(Some(bytes)) => cover = Some(bytes),
                Ok(None) => warning = " No front cover available for this edition.".into(),
                Err(e) => warning = format!(" Cover unavailable: {e} Tags can still be reviewed."),
            }
        }
        let provenance = format!(
            "Reviewed MusicBrainz recording {} / release {}. {}{}",
            row.recording, row.release, row.evidence, warning
        );
        Ok(DiscoveryProposal {
            track: row.track_id,
            candidate: id,
            values,
            hash,
            identity,
            provenance,
            cover,
        })
    }
    fn download_cover(&self, row: &Candidate, fetch: &impl Fetch) -> Result<Option<Vec<u8>>> {
        let url = format!(
            "https://coverartarchive.org/release/{}/front-500",
            row.release
        );
        self.provider_bytes(&url, ART_LIMIT, true, fetch)
    }
    /// Caller serializes this publication/preparation with library scans and other metadata preparations.
    pub fn prepare_discovery(
        &self,
        mut proposal: DiscoveryProposal,
    ) -> Result<Vec<MetadataRecord>> {
        if self.metadata_origin(proposal.track)?
            != (proposal.hash.clone(), proposal.identity.clone())
        {
            return Err(
                "File changed during discovery. Search again; no music was changed.".into(),
            );
        }
        if let Some(bytes) = proposal.cover {
            let mut cache = artwork::Cache::new(self.artwork_dir.clone());
            match cache.store(&bytes) {
                Ok(cover) if !cover.is_empty() => {
                    self.db
                        .execute(
                            "UPDATE metadata_candidates SET artwork=? WHERE id=?",
                            params![cover, proposal.candidate],
                        )
                        .map_err(|e| e.to_string())?;
                    proposal.values.push(MetadataValue {
                        field: MetadataField::Artwork,
                        value: cover,
                    });
                }
                _ => proposal
                    .provenance
                    .push_str(" Downloaded cover was not a supported bounded JPEG/PNG; tags only."),
            }
        }
        self.prepare_metadata_origin(
            proposal.track,
            &proposal.values,
            Some((&proposal.hash, &proposal.identity, &proposal.provenance)),
        )
    }
}
fn candidate_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Candidate> {
    Ok(Candidate {
        id: r.get(0)?,
        track_id: r.get(1)?,
        flags: r.get(2)?,
        score: r.get(3)?,
        duration: r.get(4)?,
        title: r.get(5)?,
        artist: r.get(6)?,
        album: r.get(7)?,
        date: r.get(8)?,
        edition: r.get(9)?,
        evidence: r.get(10)?,
        source: r.get(11)?,
        artwork: r.get(12)?,
        recording: r.get(13)?,
        release: r.get(14)?,
    })
}

#[cfg(test)]
#[path = "providers_tests.rs"]
mod tests;
