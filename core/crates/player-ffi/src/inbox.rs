use super::*;

/// # Safety
/// Live core; 0 reads status, 1 enables an existing folder, 2 disables it.
/// Snapshot uses the existing owned folder/message layout; free with spiewak_snapshot_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_inbox(ptr: *mut Core, folder: i64, action: u32) -> *mut Snapshot {
    guard(|| {
        let c = unsafe { core(ptr) }?;
        if action > 2 {
            return Err("Unknown Inbox action.".into());
        }
        if action != 0 {
            if c.scanning.swap(true, Ordering::AcqRel) {
                return Err("A library operation is running. Try again when it finishes.".into());
            }
            let _scan = ScanGuard(&c.scanning);
            Library::open(&c.path)?.set_inbox(folder, action == 1)?;
            c.monitor
                .notice("Inbox folders updated. Existing music was not changed.");
        }
        let (folders, message) = Library::open(&c.path)?.inbox_status()?;
        let mut result = Snapshot::default().folders(folders);
        result.message = result.string(&message);
        Ok(result)
    })
}

/// # Safety
/// Live core and an Inbox Track ID/event_time pair returned by listening view 3.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spiewak_inbox_complete(
    ptr: *mut Core,
    track: i64,
    observed: i64,
) -> *mut Snapshot {
    guard(|| {
        let c = unsafe { core(ptr) }?;
        if c.scanning.swap(true, Ordering::AcqRel) {
            return Err("A library operation is running. Try again when it finishes.".into());
        }
        let _scan = ScanGuard(&c.scanning);
        Library::open(&c.path)?.complete_inbox(track, observed)?;
        c.monitor
            .notice("Marked as reviewed. The file stays in its folder.");
        Ok(Snapshot::default())
    })
}
