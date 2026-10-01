//! Separately owned metadata snapshots keep the playback ABI small.
use super::*;
use player_library::{MetadataField, MetadataRecord, MetadataValue};

#[repr(C)]
pub struct FieldDto {
    field: u32,
    value: *const c_char,
}
#[repr(C)]
pub struct MetadataDto {
    operation_id: i64,
    track_id: i64,
    state: u32,
    created: i64,
    original_path: *const c_char,
    target_path: *const c_char,
    message: *const c_char,
    fields: *const FieldDto,
    field_count: usize,
}
#[repr(C)]
#[derive(Default)]
pub struct MetadataSnapshot {
    error: *const c_char,
    rows: *const MetadataDto,
    count: usize,
    strings: Vec<CString>,
    fields: Vec<Vec<FieldDto>>,
    records: Vec<MetadataDto>,
}
impl MetadataSnapshot {
    fn string(&mut self, s: &str) -> *const c_char {
        let value = CString::new(s.replace('\0', "�")).expect("NULs removed");
        let ptr = value.as_ptr();
        self.strings.push(value);
        ptr
    }
    fn records(mut self, records: Vec<MetadataRecord>) -> Self {
        for r in records {
            let original_path = self.string(&r.original_path);
            let target_path = self.string(&r.target_path);
            let message = self.string(&r.message);
            let fields = r
                .values
                .iter()
                .map(|v| FieldDto {
                    field: v.field as u32,
                    value: self.string(&v.value),
                })
                .collect::<Vec<_>>();
            self.records.push(MetadataDto {
                operation_id: r.operation_id,
                track_id: r.track_id,
                state: r.state as u32,
                created: r.created,
                original_path,
                target_path,
                message,
                fields: fields.as_ptr(),
                field_count: fields.len(),
            });
            self.fields.push(fields);
        }
        self.rows = self.records.as_ptr();
        self.count = self.records.len();
        self
    }
}
pub(super) fn result(
    action: impl FnOnce() -> Result<Vec<MetadataRecord>, String>,
) -> *mut MetadataSnapshot {
    let mut snapshot = MetadataSnapshot::default();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)) {
        Ok(Ok(rows)) => snapshot = snapshot.records(rows),
        Ok(Err(e)) => snapshot.error = snapshot.string(&e),
        Err(_) => snapshot.error = snapshot.string(
            "The metadata operation encountered an internal error. Restart the player to reconcile the journal; original backups are retained.",
        ),
    }
    Box::into_raw(Box::new(snapshot))
}
pub(super) fn library(
    c: &Core,
    action: impl FnOnce(&Library) -> Result<Vec<MetadataRecord>, String>,
) -> Result<Vec<MetadataRecord>, String> {
    if c.scanning.swap(true, Ordering::AcqRel) {
        return Err("A library operation is running. Try again when it finishes.".into());
    }
    let _scan = ScanGuard(&c.scanning);
    action(&Library::open(&c.path)?)
}
/// # Safety
/// A snapshot from these metadata APIs, released exactly once after reading its rows/strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_metadata_free(ptr: *mut MetadataSnapshot) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr) });
    }
}
/// # Safety
/// Live core. View 0 reads a track; 1 lists latest 100 operations; 2 proposes conservative download-name cleanup.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_metadata(
    ptr: *mut Core,
    id: i64,
    view: u32,
) -> *mut MetadataSnapshot {
    result(|| {
        library(unsafe { core(ptr) }?, |lib| match view {
            0 => Ok(vec![lib.metadata(id)?]),
            1 => lib.metadata_operations(),
            2 => {
                let mut row = lib.metadata(id)?;
                row.values = lib.cleanup_suggestion(id)?;
                Ok(vec![row])
            }
            _ => Err("Unknown metadata view.".into()),
        })
    })
}
/// # Safety
/// Live core and 1..18 valid typed fields with NUL-terminated UTF-8 strings, borrowed only for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_metadata_prepare(
    ptr: *mut Core,
    track: i64,
    fields: *const FieldDto,
    count: usize,
) -> *mut MetadataSnapshot {
    result(|| {
        if fields.is_null() || count == 0 || count > 18 {
            return Err("Choose between 1 and 18 metadata fields.".into());
        }
        let edits = unsafe { std::slice::from_raw_parts(fields, count) }
            .iter()
            .map(|f| {
                Ok(MetadataValue {
                    field: MetadataField::from_value(f.field)?,
                    value: unsafe { text(f.value) }?.to_owned(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        library(unsafe { core(ptr) }?, |lib| {
            lib.prepare_metadata(track, &edits)
        })
    })
}
struct AudioEdit<'a>(&'a Audio);
impl Drop for AudioEdit<'_> {
    fn drop(&mut self) {
        let _ = self.0.command(Command::EndEdit);
    }
}
/// # Safety
/// Live core. Action 1 apply, 2 undo, 3 cancel proposal, 4 explicitly recover the original and preserve current bytes.
/// Applying/undoing the selected track unloads its decoder and leaves its queue/position paused; the user resumes explicitly.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_metadata_command(
    ptr: *mut Core,
    operation: i64,
    action: u32,
) -> *mut MetadataSnapshot {
    result(|| {
        let c = unsafe { core(ptr) }?;
        let _operation = c
            .file_operation
            .lock()
            .map_err(|_| "File operation lock unavailable.")?;
        library(c, |lib| {
            if action == 3 {
                lib.cancel_metadata(operation)?;
                return lib.metadata_operations();
            }
            if !matches!(action, 1 | 2 | 4) {
                return Err("Unknown metadata command.".into());
            }
            let track = lib.metadata_operation(operation)?.track_id;
            c.audio.command(Command::BeginEdit(track))?;
            let _audio = AudioEdit(&c.audio);
            let edit = match action {
                1 => lib.apply_metadata(operation),
                2 => lib.undo_metadata(operation),
                _ => lib.recover_metadata_original(operation),
            };
            // Refresh even after failure: a completed write may have a diagnostic, and a rollback may update its name.
            let queue = c
                .audio
                .queue()?
                .iter()
                .map(|t| {
                    lib.selected_tracks(&[t.id])
                        .ok()
                        .and_then(|r| r.into_iter().next())
                        .unwrap_or_else(|| t.clone())
                })
                .collect();
            c.audio.command(Command::Refresh(queue))?;
            c.monitor
                .notice("Metadata operation finished; file change history keeps its originals.");
            c.checkpoint(true)?;
            Ok(vec![edit?])
        })
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_layout_owns_unicode_and_typed_fields() {
        assert_eq!(std::mem::size_of::<FieldDto>(), 16);
        assert_eq!(std::mem::size_of::<MetadataDto>(), 72);
        assert_eq!(std::mem::offset_of!(MetadataSnapshot, strings), 24);
        let ptr = result(|| {
            Ok(vec![MetadataRecord {
                operation_id: 9,
                track_id: 3,
                state: player_library::OperationState::Prepared,
                created: 10,
                original_path: "Muzyka/żółć.mp3".into(),
                target_path: "Muzyka/Nowa.mp3".into(),
                message: String::new(),
                values: vec![MetadataValue {
                    field: MetadataField::Title,
                    value: "Żółć".into(),
                }],
            }])
        });
        unsafe {
            let h = &*ptr;
            assert_eq!(h.count, 1);
            assert_eq!(text((*(*h.rows).fields).value).unwrap(), "Żółć");
            spiewak_metadata_free(ptr);
        }
        let ptr = unsafe { spiewak_metadata(std::ptr::null_mut(), 0, 0) };
        unsafe {
            assert!(!(*ptr).error.is_null());
            spiewak_metadata_free(ptr);
        }
    }
}
