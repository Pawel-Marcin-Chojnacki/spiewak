use std::ffi::{c_char, c_int, c_void};
unsafe extern "C" {
    fn chromaprint_decode_fingerprint(
        input: *const c_char,
        length: c_int,
        output: *mut *mut u32,
        count: *mut c_int,
        algorithm: *mut c_int,
        base64: c_int,
    ) -> c_int;
    fn chromaprint_dealloc(ptr: *mut c_void);
}
fn main() {
    let path = std::env::args_os().nth(1).expect("audio file");
    let fp = player_library::fingerprint_file(std::path::Path::new(&path)).expect("fingerprint");
    let mut ptr = std::ptr::null_mut();
    let mut count = 0;
    let mut algorithm = 0;
    assert_eq!(
        unsafe {
            chromaprint_decode_fingerprint(
                fp.encoded.as_ptr().cast(),
                fp.encoded.len() as c_int,
                &mut ptr,
                &mut count,
                &mut algorithm,
                1,
            )
        },
        1
    );
    assert_eq!(algorithm, 1);
    assert_eq!(count as u32, fp.items);
    let raw = unsafe { std::slice::from_raw_parts(ptr, count as usize) }.to_vec();
    unsafe { chromaprint_dealloc(ptr.cast()) };
    println!(
        "{}",
        serde_json::json!({"duration":fp.duration,"fingerprint":fp.encoded,"raw":raw})
    );
}
