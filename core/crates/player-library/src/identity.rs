//! Read-only Windows identity, including creation time. Unsupported volumes use paths.
#[cfg(windows)]
pub fn file_identity(path: &std::path::Path) -> Option<String> {
    use std::{
        fs::OpenOptions,
        os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    };
    #[repr(C)]
    #[derive(Default)]
    struct FileIdInfo {
        volume: u64,
        id: [u8; 16],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandleEx(
            handle: *mut std::ffi::c_void,
            class: i32,
            info: *mut std::ffi::c_void,
            size: u32,
        ) -> i32;
    }
    // FILE_FLAG_BACKUP_SEMANTICS permits directory handles; opening never writes.
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(0x02000000)
        .open(path)
        .ok()?;
    let mut info = FileIdInfo::default();
    // FileIdInfo = 18. The API fills the 24-byte FILE_ID_INFO structure.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            18,
            (&mut info as *mut FileIdInfo).cast(),
            std::mem::size_of::<FileIdInfo>() as u32,
        )
    } == 0
        || info.id == [0; 16]
    {
        return None;
    }
    let created = file
        .metadata()
        .ok()?
        .created()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let id = u128::from_le_bytes(info.id);
    Some(format!("{:016x}:{id:032x}:{created}", info.volume))
}
#[cfg(not(windows))]
pub fn file_identity(_path: &std::path::Path) -> Option<String> {
    None
}
