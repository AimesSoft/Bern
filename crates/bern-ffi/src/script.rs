use crate::BernBuffer;
#[unsafe(no_mangle)]
pub extern "C" fn bern_script_start() {
    bern_script::host::start();
}
#[unsafe(no_mangle)]
pub extern "C" fn bern_script_stop() {
    bern_script::host::stop();
}
#[unsafe(no_mangle)]
pub extern "C" fn bern_script_poll(now_ms: f64) -> BernBuffer {
    BernBuffer::from_vec(bern_script::host::poll(now_ms).into_bytes())
}
/// # Safety
/// `bytes` points to `len` bytes for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_script_complete(bytes: *const u8, len: usize) {
    if bytes.is_null() {
        return;
    }
    if let Ok(json) = std::str::from_utf8(unsafe { std::slice::from_raw_parts(bytes, len) }) {
        bern_script::host::complete(json);
    }
}
