use super::*;
use crate::tests::{sandbox, wav};
use serde_json::json;
use std::{cell::Cell, io::Cursor};
const RECORD: &str = "11111111-1111-1111-1111-111111111111";
const RELEASE: &str = "22222222-2222-2222-2222-222222222222";
const OTHER: &str = "33333333-3333-3333-3333-333333333333";
struct Fixture {
    requests: Cell<usize>,
    live: bool,
    duplicate: bool,
    cover_status: u16,
}
impl Fixture {
    fn new() -> Self {
        Self {
            requests: Cell::new(0),
            live: false,
            duplicate: false,
            cover_status: 200,
        }
    }
}
impl Fetch for Fixture {
    fn get(&self, url: &str, _: usize) -> Result<Response> {
        self.requests.set(self.requests.get() + 1);
        let body = if url.contains("/recording/?") {
            serde_json::to_vec(&json!({"recordings":[{"id":RECORD,"title":"Song","length":100,"score":"100","disambiguation":if self.live {"live"} else {""},"artist-credit":[{"name":"Artist"}],"releases":[{"id":RELEASE,"title":"Single","country":"PL"},{"id":OTHER,"title":"Compilation","release-group":{"secondary-types":["Compilation"]}}]}]})).unwrap()
        } else if url.contains("/ws/2/release/") {
            let t = json!({"position":1,"recording":{"id":RECORD}});
            serde_json::to_vec(&json!({"id":RELEASE,"title":"Single","date":"2020-02-01","artist-credit":[{"name":"Artist"}],"media":[{"position":1,"track-count":if self.duplicate {2} else {1},"tracks":if self.duplicate {vec![t.clone(),t]} else {vec![t]}}]})).unwrap()
        } else if url.contains("coverartarchive.org") {
            return Ok(Response {
                status: if self.cover_status == 200 {
                    307
                } else {
                    self.cover_status
                },
                body: vec![],
                location: Some("http://archive.org/cover.png".into()),
                retry: None,
            });
        } else {
            assert_eq!(url, "https://archive.org/cover.png");
            let mut bytes = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                4,
                4,
                image::Rgb([20, 50, 80]),
            ))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
            bytes.into_inner()
        };
        Ok(Response {
            status: 200,
            body,
            location: None,
            retry: None,
        })
    }
}
struct Reply {
    status: u16,
    body: Vec<u8>,
    destination: Option<String>,
    calls: Cell<usize>,
}
impl Fetch for Reply {
    fn get(&self, _: &str, _: usize) -> Result<Response> {
        self.calls.set(self.calls.get() + 1);
        Ok(Response {
            status: self.status,
            body: self.body.clone(),
            location: self.destination.clone(),
            retry: Some(120),
        })
    }
}
fn indexed(name: &str) -> (PathBuf, Library, Track) {
    let p = sandbox();
    let root = p.join("Music");
    fs::create_dir(&root).unwrap();
    wav(&root.join(name));
    let lib = Library::open(&p.join("index.db")).unwrap();
    lib.add_root(&root).unwrap();
    let track = lib.search("Song").unwrap().1.remove(0);
    (p, lib, track)
}
#[test]
fn reviewed_discovery_preserves_single_folder_audio_originals_and_provenance() {
    let (p, lib, track) = indexed("Artist - Song.wav");
    let bytes = fs::read(&track.path).unwrap();
    let fixture = Fixture::new();
    assert!(!lib.discovery_enabled().unwrap());
    assert!(
        lib.discover_with(track.id, "Song", "Artist", &fixture)
            .is_err()
    );
    assert_eq!(fixture.requests.get(), 0);
    lib.set_discovery_enabled(true).unwrap();
    let rows = lib
        .discover_with(track.id, "Song", "Artist", &fixture)
        .unwrap()
        .candidates;
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|r| r.score == 100 && r.evidence.contains("not identity confidence"))
    );
    assert_eq!(fs::read(&track.path).unwrap(), bytes);
    let id = rows.iter().find(|r| r.release == RELEASE).unwrap().id;
    let proposal = lib.proposal_with(id, false, false, &fixture).unwrap();
    let preview = lib.prepare_discovery(proposal).unwrap();
    let operation = preview[1].operation_id;
    assert_eq!(fs::read(&track.path).unwrap(), bytes);
    assert!(
        preview[1]
            .values
            .iter()
            .any(|v| v.field == MetadataField::Artwork && v.value == "1 embedded picture(s)")
    );
    lib.apply_metadata(operation).unwrap();
    let updated = lib.selected_tracks(&[track.id]).unwrap().remove(0);
    assert_eq!(updated.folder_id, track.folder_id);
    assert_eq!(updated.path, track.path);
    assert_eq!(
        lib.folder(track.folder_id).unwrap().unwrap().kind,
        FolderKind::LooseTracks
    );
    assert_eq!(
        lib.metadata_operation(operation).unwrap().state,
        OperationState::Applied
    );
    assert!(
        lib.metadata_operation(operation)
            .unwrap()
            .message
            .contains(RECORD)
    );
    lib.undo_metadata(operation).unwrap();
    assert_eq!(fs::read(&track.path).unwrap(), bytes);
    assert!(
        lib.metadata_operation(operation)
            .unwrap()
            .message
            .contains(RELEASE)
    );
    let requests = fixture.requests.get();
    lib.set_discovery_enabled(false).unwrap();
    assert_eq!(
        lib.discover_with(track.id, "Song", "Artist", &fixture)
            .unwrap()
            .candidates
            .len(),
        2
    );
    assert_eq!(fixture.requests.get(), requests);
    drop(lib);
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn version_conflict_cannot_be_bypassed_by_editing_search_terms() {
    let (p, lib, track) = indexed("Artist - Song (Live).wav");
    lib.set_discovery_enabled(true).unwrap();
    let fixture = Fixture::new();
    let rows = lib
        .discover_with(track.id, "Song", "Artist", &fixture)
        .unwrap()
        .candidates;
    assert!(rows.iter().all(|r| r.flags & 8 == 0));
    assert!(lib.proposal_with(rows[0].id, true, true, &fixture).is_err());
    let mut fixture = Fixture::new();
    fixture.live = true;
    let rows = lib
        .discover_with(track.id, "Song (Live)", "Artist", &fixture)
        .unwrap()
        .candidates;
    let proposal = lib
        .proposal_with(rows[0].id, true, false, &fixture)
        .unwrap();
    assert!(
        proposal
            .values
            .iter()
            .any(|v| v.field == MetadataField::Title && v.value == "Song (Live)")
    );
    drop(lib);
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn external_changes_invalidate_proposal_and_ambiguous_positions_are_not_invented() {
    let (p, lib, track) = indexed("Artist - Song.wav");
    lib.set_discovery_enabled(true).unwrap();
    let mut fixture = Fixture::new();
    fixture.duplicate = true;
    fixture.cover_status = 404;
    let rows = lib
        .discover_with(track.id, "Song", "Artist", &fixture)
        .unwrap()
        .candidates;
    let proposal = lib
        .proposal_with(rows[0].id, false, false, &fixture)
        .unwrap();
    assert!(!proposal.values.iter().any(|v| matches!(
        v.field,
        MetadataField::TrackNumber
            | MetadataField::DiscNumber
            | MetadataField::TrackTotal
            | MetadataField::DiscTotal
    )));
    assert!(proposal.cover.is_none());
    assert!(proposal.provenance.contains("No front cover"));
    let edits = lib
        .prepare_metadata(
            track.id,
            &[MetadataValue {
                field: MetadataField::Comment,
                value: "External editor".into(),
            }],
        )
        .unwrap();
    lib.apply_metadata(edits[1].operation_id).unwrap();
    let changed = fs::read(&track.path).unwrap();
    assert!(lib.prepare_discovery(proposal).is_err());
    assert_eq!(fs::read(&track.path).unwrap(), changed);
    assert!(
        lib.proposal_with(rows[0].id, false, false, &fixture)
            .is_err()
    );
    drop(lib);
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn existing_tags_and_folder_artwork_win_unless_explicitly_replaced() {
    let (p, lib, track) = indexed("Artist - Song.wav");
    let edits = lib
        .prepare_metadata(
            track.id,
            &[MetadataValue {
                field: MetadataField::Album,
                value: "My organization".into(),
            }],
        )
        .unwrap();
    lib.apply_metadata(edits[1].operation_id).unwrap();
    let cover = p.join("local-cover.png");
    image::RgbImage::from_pixel(4, 4, image::Rgb([10, 20, 30]))
        .save(&cover)
        .unwrap();
    lib.db
        .execute(
            "UPDATE tracks SET artwork=?,embedded_art=? WHERE id=?",
            params![display_path(&cover), display_path(&cover), track.id],
        )
        .unwrap();
    lib.set_discovery_enabled(true).unwrap();
    let fixture = Fixture::new();
    let rows = lib
        .discover_with(track.id, "Song", "Artist", &fixture)
        .unwrap()
        .candidates;
    let proposal = lib
        .proposal_with(rows[0].id, false, false, &fixture)
        .unwrap();
    assert!(
        !proposal
            .values
            .iter()
            .any(|v| v.field == MetadataField::Album)
    );
    assert!(proposal.cover.is_none());
    let proposal = lib.proposal_with(rows[0].id, true, true, &fixture).unwrap();
    assert!(
        proposal
            .values
            .iter()
            .any(|v| v.field == MetadataField::Album && v.value == "Single")
    );
    assert!(proposal.cover.is_some());
    drop(lib);
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn cache_expiry_backoff_redirect_and_response_bounds_do_not_write_music() {
    let p = sandbox();
    let lib = Library::open(&p.join("index.db")).unwrap();
    lib.set_discovery_enabled(true).unwrap();
    let reply = Reply {
        status: 404,
        body: vec![],
        destination: None,
        calls: Cell::new(0),
    };
    let url = "https://coverartarchive.org/release/missing/front-500";
    assert!(lib.provider_bytes(url, 10, true, &reply).unwrap().is_none());
    lib.set_discovery_enabled(false).unwrap();
    assert!(lib.provider_bytes(url, 10, true, &reply).unwrap().is_none());
    assert_eq!(reply.calls.get(), 1);
    lib.db
        .execute("UPDATE provider_http SET expires=0", [])
        .unwrap();
    assert!(lib.provider_bytes(url, 10, true, &reply).is_err());
    assert_eq!(reply.calls.get(), 1);
    lib.set_discovery_enabled(true).unwrap();
    lib.db.execute("DELETE FROM provider_gate", []).unwrap();
    let busy = Reply {
        status: 503,
        body: vec![],
        destination: None,
        calls: Cell::new(0),
    };
    assert!(
        lib.provider_bytes("https://musicbrainz.org/busy", 10, false, &busy)
            .is_err()
    );
    assert!(
        lib.provider_bytes("https://musicbrainz.org/busy", 10, false, &busy)
            .unwrap_err()
            .contains("backing off")
    );
    assert_eq!(busy.calls.get(), 1);
    lib.db.execute("DELETE FROM provider_gate", []).unwrap();
    let redirect = Reply {
        status: 307,
        body: vec![],
        destination: Some("https://localhost/private".into()),
        calls: Cell::new(0),
    };
    assert!(
        lib.provider_bytes("https://coverartarchive.org/redirect", 10, true, &redirect)
            .unwrap_err()
            .contains("outside")
    );
    assert_eq!(redirect.calls.get(), 1);
    assert!(host("https://archive.org.evil.test/file", true).is_err());
    assert!(host("https://user@archive.org/file", true).is_err());
    assert!(host("https://archive.org:444/file", true).is_err());
    lib.db.execute("DELETE FROM provider_gate", []).unwrap();
    let oversized = Reply {
        status: 200,
        body: vec![0; 11],
        destination: None,
        calls: Cell::new(0),
    };
    assert!(
        lib.provider_bytes("https://musicbrainz.org/large", 10, false, &oversized)
            .is_err()
    );
    lib.db.execute("DELETE FROM provider_gate", []).unwrap();
    let malformed = Reply {
        status: 200,
        body: b"{ broken".to_vec(),
        destination: None,
        calls: Cell::new(0),
    };
    assert!(
        lib.provider_json("https://musicbrainz.org/bad", false, &malformed)
            .is_err()
    );
    assert_eq!(
        lib.db
            .query_row(
                "SELECT count(*) FROM provider_http WHERE url='https://musicbrainz.org/bad'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    drop(lib);
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn schema_eleven_upgrade_keeps_history_folder_and_favorites() {
    let (p, lib, track) = indexed("Artist - Song.wav");
    lib.set_favorites(&[track.id], true).unwrap();
    lib.set_folder_kind(track.folder_id, FolderKind::UserCollection as u32)
        .unwrap();
    let preview = lib
        .prepare_metadata(
            track.id,
            &[MetadataValue {
                field: MetadataField::Comment,
                value: "Keep history".into(),
            }],
        )
        .unwrap();
    let op = preview[1].operation_id;
    drop_migration(&lib.db);
    lib.db.execute_batch("PRAGMA user_version=11;").unwrap();
    drop(lib);
    let lib = Library::open(&p.join("index.db")).unwrap();
    assert!(!lib.discovery_enabled().unwrap());
    assert!(lib.favorite(track.id).unwrap());
    assert_eq!(
        lib.folder(track.folder_id).unwrap().unwrap().kind,
        FolderKind::UserCollection
    );
    assert_eq!(
        lib.metadata_operation(op).unwrap().state,
        OperationState::Prepared
    );
    lib.apply_metadata(op).unwrap();
    lib.undo_metadata(op).unwrap();
    drop(lib);
    fs::remove_dir_all(p).unwrap();
}

#[test]
fn title_retains_unknown_meaningful_annotations_and_omits_known_suffix_noise() {
    assert_eq!(
        retained_title(
            "Song",
            &[
                "Song (Piano session 2002) (Official Audio)",
                "Song (Piano session 2002)"
            ]
        ),
        "Song (Piano session 2002)"
    );
    assert_eq!(
        retained_title("Song (Live)", &["Song (Live) [Official Video]"]),
        "Song (Live)"
    );
    assert_ne!(versions("Song (Club Mix)"), versions("Song"));
}

struct AcousticFixture {
    calls: Cell<usize>,
    body: Value,
    status: u16,
    live: bool,
}
impl Fetch for AcousticFixture {
    fn get(&self, url: &str, _: usize) -> Result<Response> {
        assert!(url.starts_with("https://musicbrainz.org/ws/2/recording/"));
        self.calls.set(self.calls.get() + 1);
        Ok(Response { status:200, body:serde_json::to_vec(&json!({"id":RECORD,"title":"Song","length":20000,"disambiguation":if self.live {"live"} else {""},"artist-credit":[{"name":"Artist"}]})).unwrap(),location:None,retry:None })
    }
    fn post(&self, url: &str, body: &str, _: usize) -> Result<Response> {
        assert_eq!(url, "https://api.acoustid.org/v2/lookup");
        assert!(body.starts_with("client=fixturekey&duration=20&fingerprint=AQ"));
        assert!(body.contains("meta=recordingids"));
        self.calls.set(self.calls.get() + 1);
        Ok(Response {
            status: self.status,
            body: serde_json::to_vec(&self.body).unwrap(),
            location: None,
            retry: Some(120),
        })
    }
}
fn acoustic_indexed() -> (PathBuf, Library, Track) {
    let p = sandbox();
    let root = p.join("Songs");
    fs::create_dir(&root).unwrap();
    let rate = 11025u32;
    let n = rate * 20;
    let mut data = Vec::new();
    data.extend(b"RIFF");
    data.extend((36 + n * 2).to_le_bytes());
    data.extend(b"WAVEfmt ");
    data.extend(16u32.to_le_bytes());
    data.extend(1u16.to_le_bytes());
    data.extend(1u16.to_le_bytes());
    data.extend(rate.to_le_bytes());
    data.extend((rate * 2).to_le_bytes());
    data.extend(2u16.to_le_bytes());
    data.extend(16u16.to_le_bytes());
    data.extend(b"data");
    data.extend((n * 2).to_le_bytes());
    for i in 0..n {
        let t = i as f64 / rate as f64;
        let note = 220.0 * 2.0f64.powf((t / 0.4).floor() % 12.0 / 12.0);
        let sample = ((t * note * std::f64::consts::TAU).sin() * 10000.0) as i16;
        data.extend(sample.to_le_bytes());
    }
    fs::write(root.join("Song.wav"), data).unwrap();
    let lib = Library::open(&p.join("index.db")).unwrap();
    lib.add_root(&root).unwrap();
    let track = lib.search("Song").unwrap().1.remove(0);
    (p, lib, track)
}
#[test]
fn local_fingerprint_without_key_never_contacts_provider_and_cache_is_content_bound() {
    let (p, lib, track) = acoustic_indexed();
    let original = fs::read(&track.path).unwrap();
    let f = AcousticFixture {
        calls: Cell::new(0),
        body: json!({}),
        status: 200,
        live: false,
    };
    let result = lib.identify_with(track.id, None, &f).unwrap();
    assert!(result.message.contains("No request was sent"));
    assert_eq!(f.calls.get(), 0);
    assert!(!lib.discovery_enabled().unwrap());
    assert!(
        lib.identify_with(track.id, Some("fixturekey"), &f)
            .unwrap_err()
            .contains("Enable online")
    );
    assert_eq!(f.calls.get(), 0);
    let fp = lib.audio_fingerprint(track.id).unwrap();
    assert!((fp.duration - 20.0).abs() < 0.001);
    assert!(fp.items > 100);
    assert!(!fp.encoded.is_empty());
    assert_eq!(fs::read(&track.path).unwrap(), original);
    lib.db
        .execute("UPDATE fingerprints SET encoded='CACHE-HIT'", [])
        .unwrap();
    assert_eq!(
        lib.audio_fingerprint(track.id).unwrap().encoded,
        "CACHE-HIT"
    );
    let mut changed = original.clone();
    changed[60] ^= 1;
    fs::write(&track.path, changed).unwrap();
    assert_ne!(
        lib.audio_fingerprint(track.id).unwrap().encoded,
        "CACHE-HIT"
    );
    drop(lib);
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn acoustic_matches_are_reviewed_and_preserve_versions_source_and_provenance() {
    let (p, lib, track) = acoustic_indexed();
    let original = fs::read(&track.path).unwrap();
    lib.set_discovery_enabled(true).unwrap();
    let f = AcousticFixture {
        calls: Cell::new(0),
        body: json!({"status":"ok","results":[{"id":OTHER,"score":0.98,"recordings":[{"id":RECORD},{"id":"invalid"}]}]}),
        status: 200,
        live: false,
    };
    let result = lib.identify_with(track.id, Some("fixturekey"), &f).unwrap();
    assert_eq!(result.candidates.len(), 1);
    let row = &result.candidates[0];
    assert_ne!(row.flags & 16, 0);
    assert_ne!(row.flags & 4, 0);
    assert_ne!(row.flags & 8, 0);
    assert_eq!(row.score, 98);
    assert!(row.evidence.contains("not identity certainty"));
    assert!(row.source.contains("acoustid.org"));
    let proposal = lib.proposal_with(row.id, false, false, &f).unwrap();
    let records = lib.prepare_discovery(proposal).unwrap();
    assert_eq!(fs::read(&track.path).unwrap(), original);
    let op = records.last().unwrap().operation_id;
    lib.apply_metadata(op).unwrap();
    assert!(
        lib.metadata_operations().unwrap()[0]
            .message
            .contains("AcoustID")
    );
    lib.undo_metadata(op).unwrap();
    assert_eq!(fs::read(&track.path).unwrap(), original);
    // A live fingerprint result is not allowed to erase the absent/contradictory version marker.
    lib.db.execute("DELETE FROM provider_http", []).unwrap();
    lib.db.execute("DELETE FROM provider_gate", []).unwrap();
    let live = AcousticFixture { live: true, ..f };
    let rows = lib
        .identify_with(track.id, Some("fixturekey"), &live)
        .unwrap()
        .candidates;
    assert_eq!(rows[0].flags & 8, 0);
    assert!(lib.proposal_with(rows[0].id, false, false, &live).is_err());
    drop(lib);
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn acoustic_errors_are_static_and_rate_backoff_prevents_repeated_requests() {
    let (p, lib, track) = acoustic_indexed();
    lib.set_discovery_enabled(true).unwrap();
    let f = AcousticFixture {
        calls: Cell::new(0),
        body: json!({"status":"error","error":{"message":"fixturekey secret echo"}}),
        status: 200,
        live: false,
    };
    let e = lib
        .identify_with(track.id, Some("fixturekey"), &f)
        .unwrap_err();
    assert!(!e.contains("fixturekey"));
    assert!(e.contains("rejected"));
    assert_eq!(f.calls.get(), 1);
    assert!(
        lib.identify_with(track.id, Some("fixturekey"), &f)
            .unwrap_err()
            .contains("backing off")
    );
    assert_eq!(f.calls.get(), 1);
    lib.db.execute("DELETE FROM provider_gate", []).unwrap();
    let f = AcousticFixture { status: 429, ..f };
    assert!(
        lib.identify_with(track.id, Some("fixturekey"), &f)
            .unwrap_err()
            .contains("rate-limited")
    );
    assert_eq!(fs::read(&track.path).unwrap().len(), 44 + 20 * 11025 * 2);
    drop(lib);
    fs::remove_dir_all(p).unwrap();
}

#[test]
fn acoustic_empty_invalid_and_duplicate_matches_do_not_invent_identity() {
    let (p, lib, track) = acoustic_indexed();
    lib.set_discovery_enabled(true).unwrap();
    let original = fs::read(&track.path).unwrap();
    let f = AcousticFixture {
        calls: Cell::new(0),
        body: json!({"status":"ok","results":[{"id":OTHER,"score":0.2,"recordings":[{"id":RECORD}]},{"id":RELEASE,"score":0.9,"recordings":[{"id":RECORD}]},{"id":"bad-id","score":1.0,"recordings":[{"id":RECORD}]},{"id":OTHER,"score":2.0,"recordings":[{"id":RECORD}]}]}),
        status: 200,
        live: false,
    };
    let rows = lib
        .identify_with(track.id, Some("fixturekey"), &f)
        .unwrap()
        .candidates;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].score, 90);
    assert!(rows[0].evidence.contains("1 recording(s) returned"));
    assert!(rows[0].evidence.contains(RELEASE));
    assert_eq!(f.calls.get(), 2);
    lib.db.execute("DELETE FROM provider_gate", []).unwrap();
    let empty = AcousticFixture {
        body: json!({"status":"ok","results":[]}),
        ..f
    };
    assert!(
        lib.identify_with(track.id, Some("fixturekey"), &empty)
            .unwrap()
            .candidates
            .is_empty()
    );
    assert!(lib.discovery(track.id).unwrap().candidates.is_empty());
    assert_eq!(fs::read(&track.path).unwrap(), original);
    let short = p.join("short.wav");
    wav(&short);
    assert!(fingerprint::fingerprint_file(&short).is_err());
    fs::write(&short, b"broken audio").unwrap();
    assert!(fingerprint::fingerprint_file(&short).is_err());
    drop(lib);
    fs::remove_dir_all(p).unwrap();
}
