//! The bridge for Android: the C ABI as JNI methods of the Kotlin object
//! `dev.bern.runtime.BernNative`, and callbacks into its `BernBridge`.
//!
//! Byte arrays cross the boundary in the same compact layout as on the C
//! ABI. Callbacks run on engine and channel threads, which attach to the
//! Java VM as daemons; the Kotlin side moves the work to its main thread.
//! The generated glue defines `start`, which knows the engine; every other
//! method is here.

use crate::{
    AppSpec, BernApp, BernBuffer, BernCallbacks, bern_app_channel, bern_app_dispatch,
    bern_app_engine_id, bern_app_snapshot, bern_app_stop, bern_buffer_free, bern_session_complete,
    bern_session_set, bern_window_closed, bern_window_confirm_close, bern_window_facts,
    bern_window_opened, bern_window_request_close, media, query, start_app,
};
use bern_command::EngineProtocol;
use bern_command::files::{Opener, SourceFile};
use bern_protocol::CommandError;
use jni_sys::{
    JNI_OK, JNIEnv, JavaVM, jbyte, jbyteArray, jdouble, jint, jlong, jmethodID, jobject, jsize,
    jstring, jvalue,
};
use std::ffi::{CStr, CString, c_void};

/// The JNI types, for the generated glue's `start`.
pub use jni_sys;

/// The Kotlin bridge object and the methods the callbacks call on it.
struct Bridge {
    vm: *mut JavaVM,
    object: jobject,
    on_state: jmethodID,
    on_reply: jmethodID,
    on_session_request: jmethodID,
    on_media_request: jmethodID,
    on_media_state: jmethodID,
    on_query: jmethodID,
    file_name: jmethodID,
    open_file: jmethodID,
}

impl Bridge {
    /// The calling thread's environment, attaching it first if needed.
    unsafe fn env(&self) -> Option<*mut JNIEnv> {
        let mut env: *mut c_void = std::ptr::null_mut();
        // SAFETY: `vm` is the process's Java VM, valid for its lifetime.
        let attached = unsafe {
            ((**self.vm).v1_4.AttachCurrentThreadAsDaemon)(self.vm, &mut env, std::ptr::null_mut())
        };
        (attached == JNI_OK && !env.is_null()).then_some(env.cast())
    }

    /// Call `method` with `args` and drop any exception it threw: a failing
    /// callback must not take the engine thread down.
    unsafe fn call(&self, method: jmethodID, args: &[jvalue], array: jbyteArray) {
        // SAFETY: the method belongs to the bridge object and takes `args`.
        unsafe {
            let Some(env) = self.env() else { return };
            ((**env).v1_6.CallVoidMethodA)(env, self.object, method, args.as_ptr());
            if ((**env).v1_6.ExceptionCheck)(env) {
                ((**env).v1_6.ExceptionDescribe)(env);
                ((**env).v1_6.ExceptionClear)(env);
            }
            if !array.is_null() {
                ((**env).v1_6.DeleteLocalRef)(env, array);
            }
        }
    }

    unsafe fn bytes(&self, method: jmethodID, bytes: &[u8]) {
        // SAFETY: forwarded from the callback.
        unsafe {
            let Some(env) = self.env() else { return };
            let array = bytes_out(env, bytes);
            self.call(method, &[jvalue { l: array }], array);
        }
    }

    /// Ask the bridge about the document `uri`: `call` calls a method with
    /// the URI as a Java string and reads the result while the environment
    /// is at hand. An exception the method threw is dropped, answering
    /// `None`.
    unsafe fn ask<T>(&self, uri: &str, call: impl FnOnce(*mut JNIEnv, jvalue) -> T) -> Option<T> {
        // SAFETY: the environment belongs to this thread.
        unsafe {
            let env = self.env()?;
            // Document URIs are ASCII, where modified UTF-8 is UTF-8.
            let text = CString::new(uri).ok()?;
            let argument = ((**env).v1_6.NewStringUTF)(env, text.as_ptr());
            if argument.is_null() {
                ((**env).v1_6.ExceptionClear)(env);
                return None;
            }
            let answer = call(env, jvalue { l: argument });
            let thrown = ((**env).v1_6.ExceptionCheck)(env);
            if thrown {
                ((**env).v1_6.ExceptionDescribe)(env);
                ((**env).v1_6.ExceptionClear)(env);
            }
            ((**env).v1_6.DeleteLocalRef)(env, argument);
            (!thrown).then_some(answer)
        }
    }

    unsafe fn numbered(&self, method: jmethodID, number: u64, bytes: &[u8]) {
        // SAFETY: forwarded from the callback.
        unsafe {
            let Some(env) = self.env() else { return };
            let array = bytes_out(env, bytes);
            self.call(
                method,
                &[jvalue { j: number as jlong }, jvalue { l: array }],
                array,
            );
        }
    }
}

/// # Safety
/// `context` is the leaked [`Bridge`] of `start`; `bytes` points to `len`
/// readable bytes.
unsafe fn bridge<'a>(context: *mut c_void) -> &'a Bridge {
    // SAFETY: guaranteed by the caller.
    unsafe { &*context.cast::<Bridge>() }
}

extern "C" fn on_state(context: *mut c_void, bytes: *const u8, len: usize) {
    // SAFETY: the runtime passes back the context of `start`.
    unsafe {
        let bridge = bridge(context);
        bridge.bytes(bridge.on_state, crate::slice(bytes, len));
    }
}

extern "C" fn on_reply(context: *mut c_void, token: u64, bytes: *const u8, len: usize) {
    // SAFETY: as above.
    unsafe {
        let bridge = bridge(context);
        bridge.numbered(bridge.on_reply, token, crate::slice(bytes, len));
    }
}

extern "C" fn on_session_request(context: *mut c_void, request: u64, bytes: *const u8, len: usize) {
    // SAFETY: as above.
    unsafe {
        let bridge = bridge(context);
        bridge.numbered(bridge.on_session_request, request, crate::slice(bytes, len));
    }
}

extern "C" fn on_media_request(context: *mut c_void, bytes: *const u8, len: usize) {
    // SAFETY: as above.
    unsafe {
        let bridge = bridge(context);
        bridge.bytes(bridge.on_media_request, crate::slice(bytes, len));
    }
}

extern "C" fn on_media_state(context: *mut c_void, bytes: *const u8, len: usize) {
    // SAFETY: as above.
    unsafe {
        let bridge = bridge(context);
        bridge.bytes(bridge.on_media_state, crate::slice(bytes, len));
    }
}

extern "C" fn on_query(context: *mut c_void, subscription: u64, bytes: *const u8, len: usize) {
    // SAFETY: as above.
    unsafe {
        let bridge = bridge(context);
        bridge.numbered(bridge.on_query, subscription, crate::slice(bytes, len));
    }
}

/// The documents the system hands the application (`content:` URIs),
/// opened through the Kotlin bridge's content resolver.
struct Documents(*mut Bridge);

// SAFETY: the bridge lives as long as the process, its object is a global
// reference, and every call attaches the calling thread first.
unsafe impl Send for Documents {}
unsafe impl Sync for Documents {}

impl Opener for Documents {
    fn name(&self, url: &str) -> Option<String> {
        // SAFETY: the bridge outlives every engine thread.
        let bridge = unsafe { &*self.0 };
        // SAFETY: `fileName` takes a string and answers a string or null.
        let name = unsafe {
            bridge.ask(url, |env, argument| {
                let name: jstring = ((**env).v1_6.CallObjectMethodA)(
                    env,
                    bridge.object,
                    bridge.file_name,
                    &argument,
                );
                if name.is_null() {
                    return None;
                }
                let len = ((**env).v1_6.GetStringLength)(env, name).max(0);
                let mut units = vec![0u16; len as usize];
                ((**env).v1_6.GetStringRegion)(env, name, 0, len, units.as_mut_ptr());
                ((**env).v1_6.DeleteLocalRef)(env, name);
                Some(String::from_utf16_lossy(&units))
            })
        };
        name.flatten()
    }

    fn open(&self, url: &str) -> Result<SourceFile, CommandError> {
        use std::os::fd::FromRawFd;
        // SAFETY: as above.
        let bridge = unsafe { &*self.0 };
        // SAFETY: `openFile` takes a string and answers an int.
        let fd = unsafe {
            bridge.ask(url, |env, argument| {
                ((**env).v1_6.CallIntMethodA)(env, bridge.object, bridge.open_file, &argument)
            })
        };
        match fd {
            Some(fd) if fd >= 0 => {
                // SAFETY: the bridge detached the descriptor for us to own.
                let file = unsafe { std::fs::File::from_raw_fd(fd) };
                let len = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
                Ok(SourceFile::new(file, len))
            }
            _ => Err(CommandError::new(
                "source_unavailable",
                format!(
                    "the system did not open `{url}`; it may be gone, or the permission to read it lost"
                ),
            )),
        }
    }
}

/// The bytes of a Java byte array; empty for null.
///
/// # Safety
/// `env` is the calling thread's environment and `array` a byte array or
/// null.
pub unsafe fn bytes_in(env: *mut JNIEnv, array: jbyteArray) -> Vec<u8> {
    if array.is_null() {
        return Vec::new();
    }
    // SAFETY: guaranteed by the caller.
    unsafe {
        let len = ((**env).v1_6.GetArrayLength)(env, array).max(0) as usize;
        let mut bytes = vec![0u8; len];
        ((**env).v1_6.GetByteArrayRegion)(
            env,
            array,
            0,
            len as jsize,
            bytes.as_mut_ptr().cast::<jbyte>(),
        );
        bytes
    }
}

/// A new Java byte array holding `bytes`.
///
/// # Safety
/// `env` is the calling thread's environment.
pub unsafe fn bytes_out(env: *mut JNIEnv, bytes: &[u8]) -> jbyteArray {
    // SAFETY: guaranteed by the caller.
    unsafe {
        let array = ((**env).v1_6.NewByteArray)(env, bytes.len() as jsize);
        if !array.is_null() && !bytes.is_empty() {
            ((**env).v1_6.SetByteArrayRegion)(
                env,
                array,
                0,
                bytes.len() as jsize,
                bytes.as_ptr().cast::<jbyte>(),
            );
        }
        array
    }
}

/// A Java byte array holding a buffer the runtime returned, which is freed.
unsafe fn buffer_out(env: *mut JNIEnv, buffer: BernBuffer) -> jbyteArray {
    // SAFETY: the buffer comes from the runtime and is freed once copied.
    unsafe {
        let bytes = crate::slice(buffer.ptr, buffer.len);
        let array = bytes_out(env, bytes);
        bern_buffer_free(buffer);
        array
    }
}

fn app(handle: jlong) -> *mut BernApp {
    handle as usize as *mut BernApp
}

/// Write a trace line to logcat under the tag `bern-trace`.
fn logcat(line: &str) {
    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(
            priority: std::os::raw::c_int,
            tag: *const std::os::raw::c_char,
            text: *const std::os::raw::c_char,
        ) -> std::os::raw::c_int;
    }
    const INFO: std::os::raw::c_int = 4;
    let line = line.strip_prefix("bern-trace: ").unwrap_or(line);
    let Ok(text) = std::ffi::CString::new(line.replace('\0', " ")) else {
        return;
    };
    // SAFETY: both strings are NUL-terminated and live for the call.
    unsafe { __android_log_write(INFO, c"bern-trace".as_ptr(), text.as_ptr()) };
}

/// Start the engine for the Kotlin frontend: `BernNative.start`, defined by
/// the generated glue, calls this. Answers `0, handle` or `1, message`.
///
/// # Safety
/// `env` is the calling thread's environment, `resources` a byte array or
/// null and `bridge` a `dev.bern.runtime.BernBridge`.
pub unsafe fn start<E: EngineProtocol>(
    env: *mut JNIEnv,
    make_engine: impl FnOnce() -> E,
    spec: AppSpec,
    fingerprint: jlong,
    resources: jbyteArray,
    bridge: jobject,
) -> jbyteArray {
    // SAFETY: guaranteed by the caller.
    unsafe {
        let fail = |message: &str| {
            let mut bytes = vec![1u8];
            bytes.extend((message.len() as u32).to_le_bytes());
            bytes.extend(message.as_bytes());
            bytes_out(env, &bytes)
        };
        let mut vm: *mut JavaVM = std::ptr::null_mut();
        if ((**env).v1_6.GetJavaVM)(env, &mut vm) != JNI_OK || vm.is_null() {
            return fail("the Java VM is not available");
        }
        let class = ((**env).v1_6.GetObjectClass)(env, bridge);
        let method = |name: &CStr, signature: &CStr| {
            ((**env).v1_6.GetMethodID)(env, class, name.as_ptr(), signature.as_ptr())
        };
        let methods = [
            method(c"onState", c"([B)V"),
            method(c"onReply", c"(J[B)V"),
            method(c"onSessionRequest", c"(J[B)V"),
            method(c"onMediaRequest", c"([B)V"),
            method(c"onMediaState", c"([B)V"),
            method(c"onQuery", c"(J[B)V"),
            method(c"fileName", c"(Ljava/lang/String;)Ljava/lang/String;"),
            method(c"openFile", c"(Ljava/lang/String;)I"),
        ];
        if methods.iter().any(|method| method.is_null()) {
            ((**env).v1_6.ExceptionClear)(env);
            return fail("the bridge object lacks a callback method");
        }
        let object = ((**env).v1_6.NewGlobalRef)(env, bridge);
        // The bridge lives as long as the process, like the engine.
        let context = Box::into_raw(Box::new(Bridge {
            vm,
            object,
            on_state: methods[0],
            on_reply: methods[1],
            on_session_request: methods[2],
            on_media_request: methods[3],
            on_media_state: methods[4],
            on_query: methods[5],
            file_name: methods[6],
            open_file: methods[7],
        }));
        bern_command::files::install("content", Documents(context));
        // Trace summaries go to logcat: an app has no terminal.
        bern_command::trace::set_sink(std::sync::Arc::new(logcat));
        let callbacks = BernCallbacks {
            context: context.cast(),
            on_state,
            on_reply,
            on_session_request,
            on_media_request,
            on_media_state,
            on_query,
        };
        let resources = bytes_in(env, resources);
        let mut error = BernBuffer::empty();
        let started = start_app(
            make_engine,
            spec,
            fingerprint as u64,
            resources.as_ptr(),
            resources.len(),
            callbacks,
            &mut error,
        );
        if started.is_null() {
            let message = String::from_utf8_lossy(crate::slice(error.ptr, error.len)).into_owned();
            bern_buffer_free(error);
            return fail(&message);
        }
        bern_buffer_free(error);
        let mut bytes = vec![0u8];
        bytes.extend((started as usize as u64).to_le_bytes());
        bytes_out(env, &bytes)
    }
}

/// Exports one JNI method of `dev.bern.runtime.BernNative`.
macro_rules! native {
    ($name:ident ($env:ident $(, $arg:ident : $ty:ty)*) -> $ret:ty $body:block) => {
        /// A method of `BernNative`.
        ///
        /// # Safety
        /// Called by the Java VM with the arguments of the Kotlin
        /// declaration and a handle from `start`.
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub unsafe extern "system" fn $name(
            $env: *mut JNIEnv,
            _this: jobject,
            $($arg: $ty),*
        ) -> $ret {
            // SAFETY: the Java VM calls with valid arguments of the declared
            // Kotlin signature; handles come from `start`.
            unsafe { $body }
        }
    };
}

native!(Java_dev_bern_runtime_BernNative_stop(_env, handle: jlong) -> () {
    bern_app_stop(app(handle))
});

native!(Java_dev_bern_runtime_BernNative_snapshot(env, handle: jlong) -> jbyteArray {
    buffer_out(env, bern_app_snapshot(app(handle)))
});

native!(Java_dev_bern_runtime_BernNative_engineId(env, handle: jlong) -> jbyteArray {
    buffer_out(env, bern_app_engine_id(app(handle)))
});

native!(Java_dev_bern_runtime_BernNative_channel(env, handle: jlong) -> jbyteArray {
    buffer_out(env, bern_app_channel(app(handle)))
});

native!(Java_dev_bern_runtime_BernNative_dispatch(
    env, handle: jlong, command: jint, params: jbyteArray, token: jlong
) -> () {
    let params = bytes_in(env, params);
    bern_app_dispatch(app(handle), command as u32, params.as_ptr(), params.len(), token as u64)
});

native!(Java_dev_bern_runtime_BernNative_windowOpened(env, handle: jlong, kind: jint) -> jbyteArray {
    buffer_out(env, bern_window_opened(app(handle), kind as u32))
});

native!(Java_dev_bern_runtime_BernNative_windowClosed(
    _env, handle: jlong, kind: jint, generation: jlong
) -> () {
    bern_window_closed(app(handle), kind as u32, generation as u64)
});

native!(Java_dev_bern_runtime_BernNative_windowFacts(
    _env,
    handle: jlong,
    kind: jint,
    generation: jlong,
    source: jint,
    visibility: jint,
    size: jint,
    x: jdouble,
    y: jdouble,
    width: jdouble,
    height: jdouble,
    focused: jint,
    actions: jint
) -> jlong {
    bern_window_facts(
        app(handle),
        kind as u32,
        generation as u64,
        source as u8,
        visibility as u8,
        size as u8,
        x,
        y,
        width,
        height,
        focused as u8,
        actions as u32,
    ) as jlong
});

native!(Java_dev_bern_runtime_BernNative_windowRequestClose(
    env, handle: jlong, kind: jint, generation: jlong, needs_confirmation: jint, can_save: jint
) -> jbyteArray {
    buffer_out(
        env,
        bern_window_request_close(
            app(handle),
            kind as u32,
            generation as u64,
            needs_confirmation as u8,
            can_save as u8,
        ),
    )
});

native!(Java_dev_bern_runtime_BernNative_windowConfirmClose(
    env, handle: jlong, kind: jint, generation: jlong, bytes: jbyteArray
) -> jbyteArray {
    let bytes = bytes_in(env, bytes);
    buffer_out(
        env,
        bern_window_confirm_close(app(handle), kind as u32, generation as u64, bytes.as_ptr(), bytes.len()),
    )
});

native!(Java_dev_bern_runtime_BernNative_sessionSet(
    env, handle: jlong, kind: jint, generation: jlong, session: jint, field: jint, value: jbyteArray
) -> jbyteArray {
    let value = bytes_in(env, value);
    buffer_out(
        env,
        bern_session_set(
            app(handle),
            kind as u32,
            generation as u64,
            session as u32,
            field as u32,
            value.as_ptr(),
            value.len(),
        ),
    )
});

native!(Java_dev_bern_runtime_BernNative_sessionComplete(
    env, handle: jlong, request: jlong, bytes: jbyteArray
) -> () {
    let bytes = bytes_in(env, bytes);
    bern_session_complete(app(handle), request as u64, bytes.as_ptr(), bytes.len())
});

native!(Java_dev_bern_runtime_BernNative_queryOpen(
    env, handle: jlong, kind: jint, generation: jlong, index: jint
) -> jbyteArray {
    buffer_out(
        env,
        query::bern_query_open(app(handle), kind as u32, generation as u64, index as u32),
    )
});

native!(Java_dev_bern_runtime_BernNative_queryRequest(
    env, handle: jlong, subscription: jlong, params: jbyteArray
) -> jbyteArray {
    let params = bytes_in(env, params);
    buffer_out(
        env,
        query::bern_query_request(app(handle), subscription as u64, params.as_ptr(), params.len()),
    )
});

native!(Java_dev_bern_runtime_BernNative_queryClose(_env, handle: jlong, subscription: jlong) -> () {
    query::bern_query_close(app(handle), subscription as u64)
});

native!(Java_dev_bern_runtime_BernNative_mediaExecutorAttached(
    env, handle: jlong, bytes: jbyteArray
) -> jbyteArray {
    let bytes = bytes_in(env, bytes);
    buffer_out(
        env,
        media::bern_media_executor_attached(app(handle), bytes.as_ptr(), bytes.len()),
    )
});

native!(Java_dev_bern_runtime_BernNative_mediaSnapshot(env, handle: jlong) -> jbyteArray {
    buffer_out(env, media::bern_media_snapshot(app(handle)))
});

native!(Java_dev_bern_runtime_BernNative_mediaDispatch(
    env, handle: jlong, command: jint, params: jbyteArray, token: jlong
) -> () {
    let params = bytes_in(env, params);
    media::bern_media_dispatch(app(handle), command as u32, params.as_ptr(), params.len(), token as u64)
});

native!(Java_dev_bern_runtime_BernNative_mediaCreate(env, handle: jlong, bytes: jbyteArray) -> jbyteArray {
    let bytes = bytes_in(env, bytes);
    buffer_out(env, media::bern_media_create(app(handle), bytes.as_ptr(), bytes.len()))
});

native!(Java_dev_bern_runtime_BernNative_mediaBind(env, handle: jlong, bytes: jbyteArray) -> jbyteArray {
    let bytes = bytes_in(env, bytes);
    buffer_out(env, media::bern_media_bind(app(handle), bytes.as_ptr(), bytes.len()))
});

native!(Java_dev_bern_runtime_BernNative_mediaRelease(env, handle: jlong, bytes: jbyteArray) -> () {
    let bytes = bytes_in(env, bytes);
    media::bern_media_release(app(handle), bytes.as_ptr(), bytes.len())
});

native!(Java_dev_bern_runtime_BernNative_mediaView(env, handle: jlong, bytes: jbyteArray) -> () {
    let bytes = bytes_in(env, bytes);
    media::bern_media_view(app(handle), bytes.as_ptr(), bytes.len())
});

native!(Java_dev_bern_runtime_BernNative_mediaComplete(env, handle: jlong, bytes: jbyteArray) -> () {
    let bytes = bytes_in(env, bytes);
    media::bern_media_complete(app(handle), bytes.as_ptr(), bytes.len())
});

native!(Java_dev_bern_runtime_BernNative_mediaEvent(env, handle: jlong, bytes: jbyteArray) -> () {
    let bytes = bytes_in(env, bytes);
    media::bern_media_event(app(handle), bytes.as_ptr(), bytes.len())
});
