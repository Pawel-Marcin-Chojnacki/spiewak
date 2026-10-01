//! Local Chromaprint calculation. No source writes, subprocesses or audio device required.
use super::*;
use std::{
    ffi::{CStr, c_char, c_int, c_void},
    io::ErrorKind,
};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions,
    io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};
const VERSION: &str = "chromaprint-1.6.1-test2-kissfft-symphonia-0.5.5-gapless-120s";
const MAX_SECONDS: f64 = 3.0 * 3600.0;
const KEY_ENV: &str = "SPIEWAK_ACOUSTID_CLIENT_KEY";
#[derive(Debug, Clone)]
pub struct Fingerprint {
    pub encoded: String,
    pub duration: f64,
    pub items: u32,
}
unsafe extern "C" {
    fn chromaprint_new(algorithm: c_int) -> *mut c_void;
    fn chromaprint_free(ctx: *mut c_void);
    fn chromaprint_start(ctx: *mut c_void, rate: c_int, channels: c_int) -> c_int;
    fn chromaprint_feed(ctx: *mut c_void, samples: *const i16, count: c_int) -> c_int;
    fn chromaprint_finish(ctx: *mut c_void) -> c_int;
    fn chromaprint_get_fingerprint(ctx: *mut c_void, output: *mut *mut c_char) -> c_int;
    fn chromaprint_get_raw_fingerprint_size(ctx: *mut c_void, size: *mut c_int) -> c_int;
    fn chromaprint_dealloc(ptr: *mut c_void);
}
struct Context(*mut c_void);
impl Drop for Context {
    fn drop(&mut self) {
        unsafe { chromaprint_free(self.0) };
    }
}
pub(super) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch("SAVEPOINT migration; CREATE TABLE fingerprints(track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,source_hash TEXT NOT NULL,source_identity TEXT NOT NULL,version TEXT NOT NULL,encoded TEXT NOT NULL,duration REAL NOT NULL,items INTEGER NOT NULL,created INTEGER NOT NULL); PRAGMA user_version=13; RELEASE migration;").map_err(|e|e.to_string())
}
#[cfg(test)]
pub(super) fn drop_migration(db: &Connection) {
    db.execute_batch("DROP TABLE IF EXISTS fingerprints")
        .unwrap();
}
pub(super) fn application_key() -> Result<Option<String>> {
    match std::env::var(KEY_ENV) {
        Ok(key) if (8..=128).contains(&key.len()) && key.bytes().all(|b| b.is_ascii_alphanumeric()) => Ok(Some(key)),
        Ok(_) => Err("The AcoustID application key in the Windows environment is invalid. No key is displayed or sent.".into()),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => Err("The AcoustID application key is not valid UTF-8.".into()),
    }
}
pub(super) fn key_configured() -> bool {
    application_key().ok().flatten().is_some()
}
/// Streams decoded PCM into the official in-process Chromaprint library; bounded first 120 seconds.
/// Full decoded duration is measured because some formats have no reliable container duration.
pub fn fingerprint_file(path: &Path) -> Result<Fingerprint> {
    let source = metadata::lock(path, false)?;
    if source.metadata().map_err(|e| e.to_string())?.len() > 512 * 1024 * 1024 {
        return Err("Audio fingerprinting supports regular files up to 512 MiB.".into());
    }
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probe = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|_| "Cannot read this audio format for identification.")?;
    let mut format = probe.format;
    let track = format
        .default_track()
        .ok_or("No audio track to identify.")?;
    let id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|_| "This audio codec cannot be identified.")?;
    let context = Context(unsafe { chromaprint_new(1) });
    if context.0.is_null() {
        return Err("Cannot allocate the audio fingerprint calculator.".into());
    }
    let mut specification = None;
    let mut duration = 0.0;
    let mut fed_frames = 0u64;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(_) => {
                return Err(
                    "Audio could not be read completely; no identification was sent.".into(),
                );
            }
        };
        if packet.track_id() != id {
            continue;
        }
        let audio = decoder
            .decode(&packet)
            .map_err(|_| "Audio decoding failed; no identification was sent.")?;
        let spec = *audio.spec();
        let channels = spec.channels.count();
        if !(1001..=192000).contains(&spec.rate) || !(1..=8).contains(&channels) {
            return Err("Unsupported audio rate/channel count for identification.".into());
        }
        if let Some(old) = specification {
            if old != spec {
                return Err(
                    "Audio format changes inside this file; identification was stopped.".into(),
                );
            }
        } else {
            if unsafe { chromaprint_start(context.0, spec.rate as c_int, channels as c_int) } != 1 {
                return Err("Chromaprint could not start.".into());
            }
            specification = Some(spec);
        }
        if audio.capacity() > spec.rate as usize * 10 {
            return Err("Decoded packet exceeds the identification buffer limit.".into());
        }
        duration += audio.frames() as f64 / spec.rate as f64;
        if duration > MAX_SECONDS {
            return Err("Audio identification is limited to files up to three hours.".into());
        }
        let remaining = (spec.rate as u64 * 120).saturating_sub(fed_frames);
        let frames = (audio.frames() as u64).min(remaining) as usize;
        if frames > 0 {
            let mut samples = SampleBuffer::<i16>::new(audio.capacity() as u64, spec);
            samples.copy_interleaved_ref(audio);
            let samples = &samples.samples()[..frames * channels];
            if unsafe { chromaprint_feed(context.0, samples.as_ptr(), samples.len() as c_int) } != 1
            {
                return Err("Chromaprint could not consume the audio.".into());
            }
            fed_frames += frames as u64;
        }
    }
    if duration < 10.0 || unsafe { chromaprint_finish(context.0) } != 1 {
        return Err(
            "At least ten seconds of decodable audio are needed for identification.".into(),
        );
    }
    let mut size = 0;
    if unsafe { chromaprint_get_raw_fingerprint_size(context.0, &mut size) } != 1 || size <= 0 {
        return Err("This audio produced no usable fingerprint.".into());
    }
    let mut ptr = std::ptr::null_mut();
    if unsafe { chromaprint_get_fingerprint(context.0, &mut ptr) } != 1 || ptr.is_null() {
        return Err("Chromaprint could not encode its fingerprint.".into());
    }
    // The upstream API returns an owned NUL-terminated ASCII string, released by its allocator.
    let encoded = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { chromaprint_dealloc(ptr.cast()) };
    if encoded.len() > 32768 {
        return Err("Fingerprint exceeds its storage limit.".into());
    }
    Ok(Fingerprint {
        encoded,
        duration,
        items: size as u32,
    })
}
impl Library {
    pub fn audio_fingerprint(&self, track: i64) -> Result<Fingerprint> {
        let (hash, identity) = self.metadata_origin(track)?;
        if let Some(fp) = self.db.query_row("SELECT encoded,duration,items FROM fingerprints WHERE track_id=? AND source_hash=? AND source_identity=? AND version=?", params![track,hash,identity,VERSION], |r|Ok(Fingerprint { encoded:r.get(0)?, duration:r.get(1)?,items:r.get(2)? })).optional().map_err(|e|e.to_string())? { return Ok(fp); }
        let path = self.selected_tracks(&[track])?.remove(0).path;
        let fp = fingerprint_file(Path::new(&path))?;
        if self.metadata_origin(track)? != (hash.clone(), identity.clone()) {
            return Err(
                "Audio changed during fingerprint calculation. Retry; no music was changed.".into(),
            );
        }
        let tx = rusqlite::Transaction::new_unchecked(
            &self.db,
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(|e| e.to_string())?;
        self.db
            .execute(
                "INSERT OR REPLACE INTO fingerprints VALUES(?,?,?,?,?,?,?,unixepoch())",
                params![
                    track,
                    hash,
                    identity,
                    VERSION,
                    fp.encoded,
                    fp.duration,
                    fp.items
                ],
            )
            .map_err(|e| e.to_string())?;
        self.db.execute("DELETE FROM fingerprints WHERE track_id NOT IN(SELECT track_id FROM fingerprints ORDER BY created DESC,track_id DESC LIMIT 5000)",[]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(fp)
    }
}
