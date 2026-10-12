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
    bern_window_opened, bern_window_request_close, query, start_app,
};
use bern_command::EngineProtocol;
use jni_sys::{
    JNI_OK, JNIEnv, JavaVM, jbyte, jbyteArray, jdouble, jint, jlong, jmethodID, jobject, jsize,
    jvalue,
};
use std::ffi::{CStr, c_void};

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
        }));
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
