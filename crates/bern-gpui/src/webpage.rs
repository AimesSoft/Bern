//! Native embedded web browser (`WebPage`) on all three desktop platforms:
//! - macOS: `WKWebView` subview attached directly to GPUI's `NSView`.
//! - Windows: `WebView2` hosted on a child `HWND` via `WebView2Loader.dll` or
//!   the installed Edge WebView2 runtime (`EmbeddedBrowserWebView.dll`).
//! - Linux: `WebKitGTK` (`libwebkitgtk-6.0.so.4` / `libwebkit2gtk-4.1.so.0`)
//!   reparented inside GPUI's X11 window (or as a synced Wayland window).

use crate::El;
use crate::ctx::{Act, Ctx};
use crate::session::{Actions, Control};
use gpui::{Bounds, IntoElement, ParentElement, Pixels, Styled, Window, canvas, div};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// Properties of a `WebPage`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebPageProps {
    pub url: String,
    pub cookies_file: String,
    pub cookie_domain: String,
    pub user_agent: String,
    /// `"auto"`, `"mobile"`, or `"desktop"`.
    pub content_mode: &'static str,
    /// CSS selector whose click closes the page (`bern-web-page://close`).
    pub close_on: String,
    pub label: String,
}

const CLOSE_SCHEME: &str = "bern-web-page";

/// Parse `name=value; name=value` cookies from `cookies_file`, filtering standard attributes.
pub(crate) fn parse_cookies(file: &str) -> Vec<(String, String)> {
    if file.is_empty() {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for part in text.split(';') {
        let trimmed = part.trim();
        let Some((name, value)) = trimmed.split_once('=') else {
            continue;
        };
        let name = name.trim();
        let value = value.trim();
        if name.is_empty()
            || matches!(
                name.to_ascii_lowercase().as_str(),
                "path" | "domain" | "expires" | "max-age" | "secure" | "httponly" | "samesite"
            )
        {
            continue;
        }
        out.push((name.to_string(), value.to_string()));
    }
    out
}

/// Build the document-start JS listener intercepting clicks on `close_on`.
pub(crate) fn close_script(selector: &str) -> String {
    let literal = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "document.addEventListener('click',function(e){{var t=e.target;if(t&&t.closest&&t.closest({literal})){{e.stopPropagation();e.preventDefault();location.href='{CLOSE_SCHEME}://close'}}}},true);"
    )
}

/// Only normal web schemes are allowed to navigate.
#[allow(dead_code)]
pub(crate) fn is_allowed_scheme(scheme: &str) -> bool {
    matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "about" | "blob" | "data"
    )
}

struct LivePage {
    window_id: String,
    props: WebPageProps,
    on_close: Option<(Act, Ctx)>,
    native: platform::NativeWebView,
}

thread_local! {
    static PAGES: RefCell<HashMap<String, LivePage>> = RefCell::new(HashMap::new());
    static SEEN: RefCell<HashMap<String, HashSet<String>>> = RefCell::new(HashMap::new());
}

/// Begin tracking mounted `WebPage` views for `window_id` on this render frame.
pub(crate) fn begin(window_id: &str) {
    SEEN.with(|seen| {
        seen.borrow_mut()
            .entry(window_id.to_string())
            .or_default()
            .clear();
    });
}

/// Destroy any `WebPage` instances in `window_id` that were not rendered this frame
/// (or all of them when `closing` is true).
pub(crate) fn end(window_id: &str, closing: bool) {
    let alive = if closing {
        SEEN.with(|seen| {
            seen.borrow_mut().remove(window_id);
        });
        HashSet::new()
    } else {
        SEEN.with(|seen| seen.borrow().get(window_id).cloned().unwrap_or_default())
    };
    PAGES.with(|pages| {
        let mut pages = pages.borrow_mut();
        let gone: Vec<String> = pages
            .iter()
            .filter(|(key, page)| page.window_id == window_id && !alive.contains(*key))
            .map(|(key, _)| key.clone())
            .collect();
        for key in gone {
            if let Some(mut page) = pages.remove(&key) {
                page.native.destroy();
            }
        }
    });
}

/// Fire the `on_close` callback registered for `key` (invoked on GPUI's main thread).
pub(crate) fn fire_close(key: &str) {
    let cb = PAGES.with(|pages| {
        pages
            .borrow()
            .get(key)
            .and_then(|page| page.on_close.clone())
    });
    if let Some((act, ctx)) = cb {
        act(&ctx);
    }
}

fn sync_native(
    key: &str,
    window_id: &str,
    bounds: Bounds<Pixels>,
    window: &Window,
    props: &WebPageProps,
    on_close: Option<(Act, Ctx)>,
) {
    let scale = window.scale_factor();
    let x = f32::from(bounds.origin.x);
    let y = f32::from(bounds.origin.y);
    let w = f32::from(bounds.size.width).max(1.0);
    let h = f32::from(bounds.size.height).max(1.0);
    PAGES.with(|pages| {
        let mut pages = pages.borrow_mut();
        if let Some(page) = pages.get_mut(key) {
            page.on_close = on_close;
            let url_changed = page.props.url != props.url
                || page.props.cookies_file != props.cookies_file
                || page.props.cookie_domain != props.cookie_domain
                || page.props.user_agent != props.user_agent;
            if url_changed {
                page.props = props.clone();
                page.native.navigate(props);
            }
            page.native.set_bounds(window, x, y, w, h, scale);
            return;
        }
        if let Some(native) = platform::NativeWebView::create(key, window, x, y, w, h, scale, props)
        {
            pages.insert(
                key.to_string(),
                LivePage {
                    window_id: window_id.to_string(),
                    props: props.clone(),
                    on_close,
                    native,
                },
            );
        }
    });
}

/// `WebPage`: embedded native web browser view synchronized to its GPUI layout box.
pub fn web_page(c: &Ctx, id: &'static str, props: WebPageProps, action: Option<Act>) -> El {
    let window_id = c.win().id.to_string();
    let key = format!("{window_id}::{}", c.key(id));
    SEEN.with(|seen| {
        seen.borrow_mut()
            .entry(window_id.clone())
            .or_default()
            .insert(key.clone());
    });
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "web_page",
        label: props.label.clone(),
        value: Some(props.url.clone()),
        enabled: c.enabled(),
        actions: Actions::default(),
    });
    let on_close = action.map(|act| (act, c.clone()));
    let place = canvas(
        move |bounds, window, _| {
            sync_native(&key, &window_id, bounds, window, &props, on_close.clone());
        },
        |_, _, _, _| {},
    )
    .size_full();
    c.grow()
        .apply(div().relative().size_full().child(place))
        .into_any_element()
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{CLOSE_SCHEME, WebPageProps, close_script, is_allowed_scheme, parse_cookies};
    use gpui::Window;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool, NSObject};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, NSURL, NSURLRequest};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::cell::RefCell;
    use std::ffi::c_void;

    #[link(name = "WebKit", kind = "framework")]
    unsafe extern "C" {}

    #[repr(C)]
    struct DecisionBlock {
        isa: *const c_void,
        flags: i32,
        reserved: i32,
        invoke: unsafe extern "C" fn(*mut DecisionBlock, isize),
    }

    struct DelegateIvars {
        key: RefCell<String>,
    }

    define_class!(
        // SAFETY: `NSObject` has no subclassing requirements.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[ivars = DelegateIvars]
        #[name = "BernGpuiWebPageDelegate"]
        struct WebDelegate;

        impl WebDelegate {
            #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
            fn decide_policy(
                &self,
                _web_view: *mut AnyObject,
                navigation_action: *mut AnyObject,
                decision_handler: *mut c_void,
            ) {
                let mut allow = true;
                if !navigation_action.is_null() {
                    let req: *mut AnyObject = unsafe { msg_send![navigation_action, request] };
                    if !req.is_null() {
                        let url: *mut AnyObject = unsafe { msg_send![req, URL] };
                        if !url.is_null() {
                            let scheme_obj: Option<Retained<NSString>> =
                                unsafe { msg_send![url, scheme] };
                            if let Some(scheme_obj) = scheme_obj {
                                let scheme = scheme_obj.to_string();
                                if scheme == CLOSE_SCHEME {
                                    crate::host::send_web_close(self.ivars().key.borrow().clone());
                                    allow = false;
                                } else {
                                    allow = is_allowed_scheme(&scheme);
                                }
                            }
                        }
                    }
                }
                if !decision_handler.is_null() {
                    let block = decision_handler as *mut DecisionBlock;
                    // WKNavigationActionPolicy: 0 = Cancel, 1 = Allow.
                    unsafe { ((*block).invoke)(block, if allow { 1 } else { 0 }) };
                }
            }

            #[unsafe(method(webView:createWebViewWithConfiguration:forNavigationAction:windowFeatures:))]
            fn create_web_view(
                &self,
                web_view: *mut AnyObject,
                _config: *mut AnyObject,
                navigation_action: *mut AnyObject,
                _features: *mut AnyObject,
            ) -> *mut AnyObject {
                if !web_view.is_null() && !navigation_action.is_null() {
                    let target: *mut AnyObject =
                        unsafe { msg_send![navigation_action, targetFrame] };
                    if target.is_null() {
                        let req: *mut AnyObject =
                            unsafe { msg_send![navigation_action, request] };
                        if !req.is_null() {
                            let _: *mut AnyObject =
                                unsafe { msg_send![web_view, loadRequest: req] };
                        }
                    }
                }
                std::ptr::null_mut()
            }
        }
    );

    pub(super) struct NativeWebView {
        web_view: *mut AnyObject,
        _delegate: Retained<WebDelegate>,
        last_rect: (f64, f64, f64, f64),
    }

    fn ns_view_of(window: &Window) -> Option<*mut AnyObject> {
        let handle = HasWindowHandle::window_handle(window).ok()?;
        match handle.as_raw() {
            RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr() as *mut AnyObject),
            _ => None,
        }
    }

    fn frame_in_ns_view(ns_view: *mut AnyObject, x: f32, y: f32, w: f32, h: f32) -> NSRect {
        let bounds: NSRect = unsafe { msg_send![ns_view, bounds] };
        let flipped: Bool = unsafe { msg_send![ns_view, isFlipped] };
        let ny = if flipped.as_bool() {
            f64::from(y)
        } else {
            bounds.size.height - f64::from(y) - f64::from(h)
        };
        NSRect::new(
            NSPoint::new(f64::from(x), ny),
            NSSize::new(f64::from(w), f64::from(h)),
        )
    }

    fn inject_cookies_and_load(web_view: *mut AnyObject, props: &WebPageProps) {
        if !props.user_agent.is_empty() {
            let ua = NSString::from_str(&props.user_agent);
            unsafe {
                let _: () = msg_send![web_view, setCustomUserAgent: &*ua];
            }
        }
        if !props.cookie_domain.is_empty() && !props.cookies_file.is_empty() {
            let config: *mut AnyObject = unsafe { msg_send![web_view, configuration] };
            let store: *mut AnyObject = if config.is_null() {
                std::ptr::null_mut()
            } else {
                unsafe { msg_send![config, websiteDataStore] }
            };
            let cookie_store: *mut AnyObject = if store.is_null() {
                std::ptr::null_mut()
            } else {
                unsafe { msg_send![store, httpCookieStore] }
            };
            if !cookie_store.is_null()
                && let Some(dict_cls) = AnyClass::get(c"NSMutableDictionary")
                && let Some(cookie_cls) = AnyClass::get(c"NSHTTPCookie")
            {
                let k_name = NSString::from_str("Name");
                let k_value = NSString::from_str("Value");
                let k_domain = NSString::from_str("Domain");
                let k_path = NSString::from_str("Path");
                let k_secure = NSString::from_str("Secure");
                let v_domain = NSString::from_str(&props.cookie_domain);
                let v_path = NSString::from_str("/");
                let v_true = NSString::from_str("TRUE");
                for (name, value) in parse_cookies(&props.cookies_file) {
                    let dict: *mut AnyObject = unsafe { msg_send![dict_cls, dictionary] };
                    let v_name = NSString::from_str(&name);
                    let v_val = NSString::from_str(&value);
                    unsafe {
                        let _: () = msg_send![dict, setObject: &*v_name, forKey: &*k_name];
                        let _: () = msg_send![dict, setObject: &*v_val, forKey: &*k_value];
                        let _: () = msg_send![dict, setObject: &*v_domain, forKey: &*k_domain];
                        let _: () = msg_send![dict, setObject: &*v_path, forKey: &*k_path];
                        let _: () = msg_send![dict, setObject: &*v_true, forKey: &*k_secure];
                        let cookie: *mut AnyObject =
                            msg_send![cookie_cls, cookieWithProperties: dict];
                        if !cookie.is_null() {
                            let nil_block: *mut c_void = std::ptr::null_mut();
                            let _: () = msg_send![cookie_store, setCookie: cookie, completionHandler: nil_block];
                        }
                    }
                }
            }
        }
        if !props.url.is_empty() {
            let url_str = NSString::from_str(&props.url);
            if let Some(url) = NSURL::URLWithString(&url_str) {
                let req = NSURLRequest::requestWithURL(&url);
                unsafe {
                    let _: *mut AnyObject = msg_send![web_view, loadRequest: &*req];
                }
            }
        }
    }

    impl NativeWebView {
        #[allow(clippy::too_many_arguments)]
        pub(super) fn create(
            key: &str,
            window: &Window,
            x: f32,
            y: f32,
            w: f32,
            h: f32,
            _scale: f32,
            props: &WebPageProps,
        ) -> Option<Self> {
            let mtm = MainThreadMarker::new()?;
            let ns_view = ns_view_of(window)?;
            let config_cls = AnyClass::get(c"WKWebViewConfiguration")?;
            let store_cls = AnyClass::get(c"WKWebsiteDataStore")?;
            let view_cls = AnyClass::get(c"WKWebView")?;

            let config: *mut AnyObject = unsafe {
                let raw: *mut AnyObject = msg_send![config_cls, alloc];
                msg_send![raw, init]
            };
            if config.is_null() {
                return None;
            }
            let ephemeral: *mut AnyObject = unsafe { msg_send![store_cls, nonPersistentDataStore] };
            if !ephemeral.is_null() {
                unsafe {
                    let _: () = msg_send![config, setWebsiteDataStore: ephemeral];
                }
            }
            // `content_mode`: 0 = recommended, 1 = mobile, 2 = desktop.
            let mode: isize = match props.content_mode {
                "mobile" => 1,
                "desktop" => 2,
                _ => 0,
            };
            if mode != 0 {
                let prefs: *mut AnyObject = unsafe { msg_send![config, defaultWebpagePreferences] };
                if !prefs.is_null() {
                    unsafe {
                        let _: () = msg_send![prefs, setPreferredContentMode: mode];
                    }
                }
            }
            if !props.close_on.is_empty()
                && let Some(script_cls) = AnyClass::get(c"WKUserScript")
            {
                let ucc: *mut AnyObject = unsafe { msg_send![config, userContentController] };
                if !ucc.is_null() {
                    let js = NSString::from_str(&close_script(&props.close_on));
                    let script: *mut AnyObject = unsafe {
                        let raw: *mut AnyObject = msg_send![script_cls, alloc];
                        msg_send![
                            raw,
                            initWithSource: &*js,
                            injectionTime: 0isize,
                            forMainFrameOnly: Bool::YES
                        ]
                    };
                    if !script.is_null() {
                        unsafe {
                            let _: () = msg_send![ucc, addUserScript: script];
                            let _: () = msg_send![script, release];
                        }
                    }
                }
            }

            let rect = frame_in_ns_view(ns_view, x, y, w, h);
            let web_view: *mut AnyObject = unsafe {
                let raw: *mut AnyObject = msg_send![view_cls, alloc];
                msg_send![raw, initWithFrame: rect, configuration: config]
            };
            unsafe {
                let _: () = msg_send![config, release];
            }
            if web_view.is_null() {
                return None;
            }
            let this = WebDelegate::alloc(mtm).set_ivars(DelegateIvars {
                key: RefCell::new(key.to_string()),
            });
            let delegate: Retained<WebDelegate> = unsafe { msg_send![super(this), init] };
            let del_ptr: *const AnyObject = (&*delegate as *const WebDelegate).cast::<AnyObject>();
            unsafe {
                let _: () = msg_send![web_view, setNavigationDelegate: del_ptr];
                let _: () = msg_send![web_view, setUIDelegate: del_ptr];
                let _: () = msg_send![ns_view, addSubview: web_view];
            }
            inject_cookies_and_load(web_view, props);
            Some(Self {
                web_view,
                _delegate: delegate,
                last_rect: (
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    rect.size.height,
                ),
            })
        }

        pub(super) fn navigate(&mut self, props: &WebPageProps) {
            inject_cookies_and_load(self.web_view, props);
        }

        pub(super) fn set_bounds(
            &mut self,
            window: &Window,
            x: f32,
            y: f32,
            w: f32,
            h: f32,
            _scale: f32,
        ) {
            let Some(ns_view) = ns_view_of(window) else {
                return;
            };
            let rect = frame_in_ns_view(ns_view, x, y, w, h);
            let tuple = (
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height,
            );
            if (tuple.0 - self.last_rect.0).abs() > 0.5
                || (tuple.1 - self.last_rect.1).abs() > 0.5
                || (tuple.2 - self.last_rect.2).abs() > 0.5
                || (tuple.3 - self.last_rect.3).abs() > 0.5
            {
                self.last_rect = tuple;
                unsafe {
                    let _: () = msg_send![self.web_view, setFrame: rect];
                }
            }
        }

        pub(super) fn destroy(&mut self) {
            if !self.web_view.is_null() {
                let nil: *mut AnyObject = std::ptr::null_mut();
                unsafe {
                    let _: () = msg_send![self.web_view, stopLoading];
                    let _: () = msg_send![self.web_view, setNavigationDelegate: nil];
                    let _: () = msg_send![self.web_view, setUIDelegate: nil];
                    let _: () = msg_send![self.web_view, removeFromSuperview];
                    let _: () = msg_send![self.web_view, release];
                }
                self.web_view = std::ptr::null_mut();
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{CLOSE_SCHEME, WebPageProps, close_script, is_allowed_scheme, parse_cookies};
    use gpui::Window;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use win32::Win32::Foundation::{E_NOINTERFACE, HWND, RECT, S_OK};
    use win32::Win32::System::Com::CoTaskMemFree;
    use win32::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
    use win32::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW,
    };
    use win32::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, HWND_TOP, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos,
        WS_CHILD, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_VISIBLE,
    };
    use win32::core::{BOOL, GUID, HRESULT, PCSTR, PCWSTR, PWSTR};

    // Standard WebView2 COM interface IIDs.
    const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);
    const IID_ENV_COMPLETED: GUID = GUID::from_u128(0x4e8a3389_c9d8_4bd2_b6b5_124fee6cc14d);
    const IID_CTRL_COMPLETED: GUID = GUID::from_u128(0x6c4819f3_c9b7_4260_8127_c9f5bde7f68c);
    const IID_NAV_STARTING: GUID = GUID::from_u128(0x9adbe429_f36d_432b_9ddc_f8881fbd76e3);
    const IID_NEW_WINDOW: GUID = GUID::from_u128(0xd4c185fe_c81c_4989_97af_2d3fa7ab5651);
    const IID_CORE_WEBVIEW2_2: GUID = GUID::from_u128(0x9e8f0cf8_e670_4b5e_b2bc_73e061e3184c);
    const IID_SETTINGS2: GUID = GUID::from_u128(0xee9a0f68_f46c_4e32_ac23_ef8cac224d2a);

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe fn com_release(ptr: *mut c_void) {
        if ptr.is_null() {
            return;
        }
        unsafe {
            let vtbl = *(ptr as *const *const *const c_void);
            let release: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*vtbl.add(2));
            release(ptr);
        }
    }

    unsafe fn com_add_ref(ptr: *mut c_void) {
        if ptr.is_null() {
            return;
        }
        unsafe {
            let vtbl = *(ptr as *const *const *const c_void);
            let add_ref: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*vtbl.add(1));
            add_ref(ptr);
        }
    }

    unsafe fn com_qi(ptr: *mut c_void, iid: &GUID) -> *mut c_void {
        if ptr.is_null() {
            return std::ptr::null_mut();
        }
        unsafe {
            let vtbl = *(ptr as *const *const *const c_void);
            let qi: unsafe extern "system" fn(
                *mut c_void,
                *const GUID,
                *mut *mut c_void,
            ) -> HRESULT = std::mem::transmute(*vtbl.add(0));
            let mut out: *mut c_void = std::ptr::null_mut();
            if qi(ptr, iid, &mut out).is_ok() {
                out
            } else {
                std::ptr::null_mut()
            }
        }
    }

    // Generic COM callback object with 4-slot vtable: QI, AddRef, Release, Invoke(arg1, arg2).
    #[repr(C)]
    struct ComCallbackVtbl {
        qi: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        invoke: unsafe extern "system" fn(*mut c_void, isize, *mut c_void) -> HRESULT,
    }

    #[repr(C)]
    struct ComCallback {
        vtbl: &'static ComCallbackVtbl,
        refs: AtomicU32,
        iid: GUID,
        func: Box<dyn FnMut(isize, *mut c_void) -> HRESULT>,
    }

    unsafe extern "system" fn cb_qi(
        this: *mut c_void,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT {
        unsafe {
            let obj = &*(this as *const ComCallback);
            if *riid == IID_IUNKNOWN || *riid == obj.iid {
                obj.refs.fetch_add(1, Ordering::Relaxed);
                *ppv = this;
                S_OK
            } else {
                *ppv = std::ptr::null_mut();
                E_NOINTERFACE
            }
        }
    }

    unsafe extern "system" fn cb_add_ref(this: *mut c_void) -> u32 {
        unsafe {
            let obj = &*(this as *const ComCallback);
            obj.refs.fetch_add(1, Ordering::Relaxed) + 1
        }
    }

    unsafe extern "system" fn cb_release(this: *mut c_void) -> u32 {
        unsafe {
            let obj = &*(this as *const ComCallback);
            let prev = obj.refs.fetch_sub(1, Ordering::Release);
            if prev == 1 {
                std::sync::atomic::fence(Ordering::Acquire);
                drop(Box::from_raw(this as *mut ComCallback));
                0
            } else {
                prev - 1
            }
        }
    }

    unsafe extern "system" fn cb_invoke(
        this: *mut c_void,
        arg1: isize,
        arg2: *mut c_void,
    ) -> HRESULT {
        unsafe {
            let obj = &mut *(this as *mut ComCallback);
            (obj.func)(arg1, arg2)
        }
    }

    static CB_VTBL: ComCallbackVtbl = ComCallbackVtbl {
        qi: cb_qi,
        add_ref: cb_add_ref,
        release: cb_release,
        invoke: cb_invoke,
    };

    fn make_callback(
        iid: GUID,
        func: impl FnMut(isize, *mut c_void) -> HRESULT + 'static,
    ) -> *mut c_void {
        Box::into_raw(Box::new(ComCallback {
            vtbl: &CB_VTBL,
            refs: AtomicU32::new(1),
            iid,
            func: Box::new(func),
        })) as *mut c_void
    }

    struct Inner {
        key: String,
        host_hwnd: HWND,
        props: WebPageProps,
        rect: (i32, i32, i32, i32),
        controller: *mut c_void,
        webview: *mut c_void,
        disposed: bool,
    }

    impl Inner {
        unsafe fn apply_bounds(&self) {
            if self.controller.is_null() {
                return;
            }
            unsafe {
                let vtbl = *(self.controller as *const *const *const c_void);
                // ICoreWebView2Controller: 0..2 IUnknown, 3 get_IsVisible, 4 put_IsVisible, 5 get_Bounds, 6 put_Bounds
                let put_visible: unsafe extern "system" fn(*mut c_void, BOOL) -> HRESULT =
                    std::mem::transmute(*vtbl.add(4));
                let put_bounds: unsafe extern "system" fn(*mut c_void, RECT) -> HRESULT =
                    std::mem::transmute(*vtbl.add(6));
                let _ = put_bounds(
                    self.controller,
                    RECT {
                        left: 0,
                        top: 0,
                        right: self.rect.2,
                        bottom: self.rect.3,
                    },
                );
                let _ = put_visible(self.controller, BOOL(1));
            }
        }

        unsafe fn configure_and_navigate(&self, initial: bool) {
            if self.webview.is_null() {
                return;
            }
            unsafe {
                let wv_vtbl = *(self.webview as *const *const *const c_void);
                // ICoreWebView2: 3 get_Settings, 5 Navigate, 7 add_NavigationStarting, 27 AddScriptToExecuteOnDocumentCreated, 41 add_NewWindowRequested
                if !self.props.cookie_domain.is_empty() && !self.props.cookies_file.is_empty() {
                    let wv2 = com_qi(self.webview, &IID_CORE_WEBVIEW2_2);
                    if !wv2.is_null() {
                        // ICoreWebView2 has 61 methods (indices 3..63); _2 adds: 64..68, 69 get_CookieManager.
                        let wv2_vtbl = *(wv2 as *const *const *const c_void);
                        let get_cm: unsafe extern "system" fn(
                            *mut c_void,
                            *mut *mut c_void,
                        ) -> HRESULT = std::mem::transmute(*wv2_vtbl.add(69));
                        let mut cm: *mut c_void = std::ptr::null_mut();
                        if get_cm(wv2, &mut cm).is_ok() && !cm.is_null() {
                            let cm_vtbl = *(cm as *const *const *const c_void);
                            // ICoreWebView2CookieManager: 3 CreateCookie, 6 AddOrUpdateCookie, 10 DeleteAllCookies
                            let create_cookie: unsafe extern "system" fn(
                                *mut c_void,
                                PCWSTR,
                                PCWSTR,
                                PCWSTR,
                                PCWSTR,
                                *mut *mut c_void,
                            )
                                -> HRESULT = std::mem::transmute(*cm_vtbl.add(3));
                            let add_cookie: unsafe extern "system" fn(
                                *mut c_void,
                                *mut c_void,
                            )
                                -> HRESULT = std::mem::transmute(*cm_vtbl.add(6));
                            let del_all: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                                std::mem::transmute(*cm_vtbl.add(10));
                            let _ = del_all(cm);
                            let dom = wide(&self.props.cookie_domain);
                            let path = wide("/");
                            for (name, val) in parse_cookies(&self.props.cookies_file) {
                                let wn = wide(&name);
                                let wv = wide(&val);
                                let mut cookie: *mut c_void = std::ptr::null_mut();
                                if create_cookie(
                                    cm,
                                    PCWSTR(wn.as_ptr()),
                                    PCWSTR(wv.as_ptr()),
                                    PCWSTR(dom.as_ptr()),
                                    PCWSTR(path.as_ptr()),
                                    &mut cookie,
                                )
                                .is_ok()
                                    && !cookie.is_null()
                                {
                                    // ICoreWebView2Cookie::put_IsSecure is at vtable index 13.
                                    let c_vtbl = *(cookie as *const *const *const c_void);
                                    let put_sec: unsafe extern "system" fn(
                                        *mut c_void,
                                        BOOL,
                                    )
                                        -> HRESULT = std::mem::transmute(*c_vtbl.add(13));
                                    let _ = put_sec(cookie, BOOL(1));
                                    let _ = add_cookie(cm, cookie);
                                    com_release(cookie);
                                }
                            }
                            com_release(cm);
                        }
                        com_release(wv2);
                    }
                }
                if !self.props.user_agent.is_empty() {
                    let get_settings: unsafe extern "system" fn(
                        *mut c_void,
                        *mut *mut c_void,
                    ) -> HRESULT = std::mem::transmute(*wv_vtbl.add(3));
                    let mut settings: *mut c_void = std::ptr::null_mut();
                    if get_settings(self.webview, &mut settings).is_ok() && !settings.is_null() {
                        let s2 = com_qi(settings, &IID_SETTINGS2);
                        if !s2.is_null() {
                            // ICoreWebView2Settings has 20 methods (indices 3..22); Settings2: 23 get_UserAgent, 24 put_UserAgent.
                            let s2_vtbl = *(s2 as *const *const *const c_void);
                            let put_ua: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT =
                                std::mem::transmute(*s2_vtbl.add(24));
                            let ua = wide(&self.props.user_agent);
                            let _ = put_ua(s2, PCWSTR(ua.as_ptr()));
                            com_release(s2);
                        }
                        com_release(settings);
                    }
                }
                if initial {
                    if !self.props.close_on.is_empty() {
                        let add_script: unsafe extern "system" fn(
                            *mut c_void,
                            PCWSTR,
                            *mut c_void,
                        )
                            -> HRESULT = std::mem::transmute(*wv_vtbl.add(27));
                        let js = wide(&close_script(&self.props.close_on));
                        let _ = add_script(self.webview, PCWSTR(js.as_ptr()), std::ptr::null_mut());
                    }
                    let key_for_nav = self.key.clone();
                    let nav_cb = make_callback(IID_NAV_STARTING, move |_sender, args| {
                        if args.is_null() {
                            return S_OK;
                        }
                        // ICoreWebView2NavigationStartingEventArgs: 3 get_Uri, 8 put_Cancel.
                        let av = *(args as *const *const *const c_void);
                        let get_uri: unsafe extern "system" fn(*mut c_void, *mut PWSTR) -> HRESULT =
                            std::mem::transmute(*av.add(3));
                        let put_cancel: unsafe extern "system" fn(*mut c_void, BOOL) -> HRESULT =
                            std::mem::transmute(*av.add(8));
                        let mut uri = PWSTR::null();
                        if get_uri(args, &mut uri).is_ok() && !uri.is_null() {
                            let s = uri.to_string().unwrap_or_default();
                            CoTaskMemFree(Some(uri.0 as *const c_void));
                            if s.starts_with(&format!("{CLOSE_SCHEME}:")) {
                                let _ = put_cancel(args, BOOL(1));
                                crate::host::send_web_close(key_for_nav.clone());
                            } else if let Some((scheme, _)) = s.split_once(':')
                                && !is_allowed_scheme(scheme)
                            {
                                let _ = put_cancel(args, BOOL(1));
                            }
                        }
                        S_OK
                    });
                    let add_nav: unsafe extern "system" fn(
                        *mut c_void,
                        *mut c_void,
                        *mut i64,
                    ) -> HRESULT = std::mem::transmute(*wv_vtbl.add(7));
                    let mut token = 0i64;
                    let _ = add_nav(self.webview, nav_cb, &mut token);
                    com_release(nav_cb);

                    let new_win_cb = make_callback(IID_NEW_WINDOW, move |sender, args| {
                        if args.is_null() {
                            return S_OK;
                        }
                        // ICoreWebView2NewWindowRequestedEventArgs: 3 get_Uri, 7 put_Handled.
                        let av = *(args as *const *const *const c_void);
                        let get_uri: unsafe extern "system" fn(*mut c_void, *mut PWSTR) -> HRESULT =
                            std::mem::transmute(*av.add(3));
                        let put_handled: unsafe extern "system" fn(*mut c_void, BOOL) -> HRESULT =
                            std::mem::transmute(*av.add(7));
                        let _ = put_handled(args, BOOL(1));
                        let mut uri = PWSTR::null();
                        if get_uri(args, &mut uri).is_ok() && !uri.is_null() {
                            let wv = sender as *mut c_void;
                            if !wv.is_null() {
                                let v = *(wv as *const *const *const c_void);
                                let nav: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT =
                                    std::mem::transmute(*v.add(5));
                                let _ = nav(wv, PCWSTR(uri.0));
                            }
                            CoTaskMemFree(Some(uri.0 as *const c_void));
                        }
                        S_OK
                    });
                    let add_nw: unsafe extern "system" fn(
                        *mut c_void,
                        *mut c_void,
                        *mut i64,
                    ) -> HRESULT = std::mem::transmute(*wv_vtbl.add(41));
                    let mut nw_token = 0i64;
                    let _ = add_nw(self.webview, new_win_cb, &mut nw_token);
                    com_release(new_win_cb);
                }
                if !self.props.url.is_empty() {
                    let navigate: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT =
                        std::mem::transmute(*wv_vtbl.add(5));
                    let url = wide(&self.props.url);
                    let _ = navigate(self.webview, PCWSTR(url.as_ptr()));
                }
            }
        }
    }

    fn read_reg_string(root: HKEY, subkey: &str, name: &str) -> Option<String> {
        let sk = wide(subkey);
        let vn = wide(name);
        let mut size = 0u32;
        let st = unsafe {
            RegGetValueW(
                root,
                PCWSTR(sk.as_ptr()),
                PCWSTR(vn.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut size),
            )
        };
        if st.is_err() || size < 2 {
            return None;
        }
        let mut buf = vec![0u16; (size as usize).div_ceil(2)];
        let st = unsafe {
            RegGetValueW(
                root,
                PCWSTR(sk.as_ptr()),
                PCWSTR(vn.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr() as *mut c_void),
                Some(&mut size),
            )
        };
        if st.is_err() {
            return None;
        }
        let nul = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..nul]))
    }

    type CreateEnvFn =
        unsafe extern "system" fn(PCWSTR, PCWSTR, *mut c_void, *mut c_void) -> HRESULT;
    type CreateEnvInternalFn =
        unsafe extern "system" fn(BOOL, i32, PCWSTR, *mut c_void, *mut c_void) -> HRESULT;

    fn create_environment(cache: &[u16], handler: *mut c_void) -> bool {
        // 1. Try WebView2Loader.dll if shipped beside the executable or on PATH.
        let loader = wide("WebView2Loader.dll");
        if let Ok(module) = unsafe { LoadLibraryW(PCWSTR(loader.as_ptr())) }
            && let Some(proc) = unsafe {
                GetProcAddress(
                    module,
                    PCSTR(c"CreateCoreWebView2EnvironmentWithOptions".as_ptr().cast()),
                )
            }
        {
            let func: CreateEnvFn = unsafe { std::mem::transmute(proc) };
            return unsafe {
                func(
                    PCWSTR::null(),
                    PCWSTR(cache.as_ptr()),
                    std::ptr::null_mut(),
                    handler,
                )
            }
            .is_ok();
        }
        // 2. Fallback: load EmbeddedBrowserWebView.dll directly from the installed Edge WebView2 runtime.
        let keys = [
            (
                HKEY_LOCAL_MACHINE,
                r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
            ),
            (
                HKEY_CURRENT_USER,
                r"Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
            ),
        ];
        #[cfg(target_arch = "aarch64")]
        let arch = "arm64";
        #[cfg(not(target_arch = "aarch64"))]
        let arch = "x64";
        for (root, subkey) in keys {
            if let Some(dir) = read_reg_string(root, subkey, "ebwebview2")
                .or_else(|| read_reg_string(root, subkey, "location"))
            {
                let dll = std::path::Path::new(&dir)
                    .join("EBWebView")
                    .join(arch)
                    .join("EmbeddedBrowserWebView.dll");
                let wdll = wide(&dll.to_string_lossy());
                if let Ok(module) = unsafe { LoadLibraryW(PCWSTR(wdll.as_ptr())) }
                    && let Some(proc) = unsafe {
                        GetProcAddress(
                            module,
                            PCSTR(
                                c"CreateWebViewEnvironmentWithOptionsInternal"
                                    .as_ptr()
                                    .cast(),
                            ),
                        )
                    }
                {
                    let func: CreateEnvInternalFn = unsafe { std::mem::transmute(proc) };
                    return unsafe {
                        func(
                            BOOL(1),
                            0,
                            PCWSTR(cache.as_ptr()),
                            std::ptr::null_mut(),
                            handler,
                        )
                    }
                    .is_ok();
                }
            }
        }
        false
    }

    pub(super) struct NativeWebView {
        inner: Rc<RefCell<Inner>>,
    }

    impl NativeWebView {
        #[allow(clippy::too_many_arguments)]
        pub(super) fn create(
            key: &str,
            window: &Window,
            x: f32,
            y: f32,
            w: f32,
            h: f32,
            scale: f32,
            props: &WebPageProps,
        ) -> Option<Self> {
            let handle = HasWindowHandle::window_handle(window).ok()?;
            let RawWindowHandle::Win32(hwin) = handle.as_raw() else {
                return None;
            };
            let parent = HWND(hwin.hwnd.get() as *mut c_void);
            let px_x = (x * scale).round() as i32;
            let px_y = (y * scale).round() as i32;
            let px_w = (w * scale).round().max(1.0) as i32;
            let px_h = (h * scale).round().max(1.0) as i32;
            let cls = wide("STATIC");
            let empty = wide("");
            let inst = unsafe { GetModuleHandleW(None).unwrap_or_default() };
            let host_hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE,
                    PCWSTR(cls.as_ptr()),
                    PCWSTR(empty.as_ptr()),
                    WS_CHILD | WS_CLIPSIBLINGS | WS_VISIBLE,
                    px_x,
                    px_y,
                    px_w,
                    px_h,
                    Some(parent),
                    None,
                    Some(inst.into()),
                    None,
                )
            }
            .ok()?;

            let inner = Rc::new(RefCell::new(Inner {
                key: key.to_string(),
                host_hwnd,
                props: props.clone(),
                rect: (px_x, px_y, px_w, px_h),
                controller: std::ptr::null_mut(),
                webview: std::ptr::null_mut(),
                disposed: false,
            }));

            let weak_env = Rc::downgrade(&inner);
            let env_cb = make_callback(IID_ENV_COMPLETED, move |hr, env_ptr| {
                if HRESULT(hr as i32).is_err() || env_ptr.is_null() {
                    return S_OK;
                }
                let Some(rc) = weak_env.upgrade() else {
                    return S_OK;
                };
                if rc.borrow().disposed {
                    return S_OK;
                }
                let host = rc.borrow().host_hwnd;
                let weak_ctrl = Rc::downgrade(&rc);
                let ctrl_cb = make_callback(IID_CTRL_COMPLETED, move |hr2, ctrl_ptr| {
                    if HRESULT(hr2 as i32).is_err() || ctrl_ptr.is_null() {
                        return S_OK;
                    }
                    let Some(rc2) = weak_ctrl.upgrade() else {
                        return S_OK;
                    };
                    let mut st = rc2.borrow_mut();
                    unsafe {
                        if st.disposed {
                            let cv = *(ctrl_ptr as *const *const *const c_void);
                            let close_fn: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                                std::mem::transmute(*cv.add(24));
                            let _ = close_fn(ctrl_ptr);
                            return S_OK;
                        }
                        com_add_ref(ctrl_ptr);
                        st.controller = ctrl_ptr;
                        // ICoreWebView2Controller::get_CoreWebView2 is at vtable index 25.
                        let cv = *(ctrl_ptr as *const *const *const c_void);
                        let get_wv: unsafe extern "system" fn(
                            *mut c_void,
                            *mut *mut c_void,
                        ) -> HRESULT = std::mem::transmute(*cv.add(25));
                        let mut wv: *mut c_void = std::ptr::null_mut();
                        if get_wv(ctrl_ptr, &mut wv).is_ok() && !wv.is_null() {
                            st.webview = wv;
                            st.apply_bounds();
                            st.configure_and_navigate(true);
                        }
                    }
                    S_OK
                });
                // ICoreWebView2Environment::CreateCoreWebView2Controller is at vtable index 3.
                unsafe {
                    let ev = *(env_ptr as *const *const *const c_void);
                    let create_ctrl: unsafe extern "system" fn(
                        *mut c_void,
                        HWND,
                        *mut c_void,
                    ) -> HRESULT = std::mem::transmute(*ev.add(3));
                    let _ = create_ctrl(env_ptr, host, ctrl_cb);
                    com_release(ctrl_cb);
                }
                S_OK
            });
            let cache_dir = std::env::temp_dir().join("BernWebPageWebView2");
            let cache_w = wide(&cache_dir.to_string_lossy());
            let started = create_environment(&cache_w, env_cb);
            unsafe { com_release(env_cb) };
            if !started {
                eprintln!("bern: WebPage requires Microsoft Edge WebView2 Runtime");
            }
            Some(Self { inner })
        }

        pub(super) fn navigate(&mut self, props: &WebPageProps) {
            let mut st = self.inner.borrow_mut();
            st.props = props.clone();
            unsafe { st.configure_and_navigate(false) };
        }

        pub(super) fn set_bounds(
            &mut self,
            _window: &Window,
            x: f32,
            y: f32,
            w: f32,
            h: f32,
            scale: f32,
        ) {
            let mut st = self.inner.borrow_mut();
            let next = (
                (x * scale).round() as i32,
                (y * scale).round() as i32,
                (w * scale).round().max(1.0) as i32,
                (h * scale).round().max(1.0) as i32,
            );
            if next != st.rect {
                st.rect = next;
                unsafe {
                    let _ = SetWindowPos(
                        st.host_hwnd,
                        Some(HWND_TOP),
                        next.0,
                        next.1,
                        next.2,
                        next.3,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                    st.apply_bounds();
                }
            }
        }

        pub(super) fn destroy(&mut self) {
            let mut st = self.inner.borrow_mut();
            st.disposed = true;
            unsafe {
                if !st.controller.is_null() {
                    let cv = *(st.controller as *const *const *const c_void);
                    let close_fn: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                        std::mem::transmute(*cv.add(24));
                    let _ = close_fn(st.controller);
                    com_release(st.controller);
                    st.controller = std::ptr::null_mut();
                }
                if !st.webview.is_null() {
                    com_release(st.webview);
                    st.webview = std::ptr::null_mut();
                }
                if !st.host_hwnd.is_invalid() {
                    let _ = DestroyWindow(st.host_hwnd);
                    st.host_hwnd = HWND::default();
                }
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod platform {
    use super::{CLOSE_SCHEME, WebPageProps, close_script, is_allowed_scheme, parse_cookies};
    use gpui::Window;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_ulong, c_void};
    use std::sync::{Mutex, OnceLock};

    enum Cmd {
        Create {
            key: String,
            parent_xid: Option<c_ulong>,
            rect: (i32, i32, i32, i32),
            props: WebPageProps,
        },
        Navigate {
            key: String,
            props: WebPageProps,
        },
        Bounds {
            key: String,
            rect: (i32, i32, i32, i32),
        },
        Destroy {
            key: String,
        },
    }

    struct GtkRuntime {
        tx: std::sync::mpsc::Sender<Cmd>,
        wake:
            unsafe extern "C" fn(unsafe extern "C" fn(*mut c_void) -> c_int, *mut c_void) -> c_uint,
    }

    static RUNTIME: OnceLock<Option<GtkRuntime>> = OnceLock::new();
    static QUEUE: OnceLock<Mutex<std::sync::mpsc::Receiver<Cmd>>> = OnceLock::new();

    unsafe fn sym<T: Copy>(lib: *mut c_void, name: &CStr) -> Option<T> {
        unsafe {
            let p = libc::dlsym(lib, name.as_ptr());
            if p.is_null() {
                None
            } else {
                Some(std::mem::transmute_copy(&p))
            }
        }
    }

    struct Api {
        gtk_init: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int,
        gtk_main: unsafe extern "C" fn(),
        gtk_window_new: unsafe extern "C" fn(c_int) -> *mut c_void,
        gtk_window_set_decorated: unsafe extern "C" fn(*mut c_void, c_int),
        gtk_window_set_default_size: unsafe extern "C" fn(*mut c_void, c_int, c_int),
        gtk_window_resize: unsafe extern "C" fn(*mut c_void, c_int, c_int),
        gtk_container_add: unsafe extern "C" fn(*mut c_void, *mut c_void),
        gtk_widget_show_all: unsafe extern "C" fn(*mut c_void),
        gtk_widget_destroy: unsafe extern "C" fn(*mut c_void),
        gtk_widget_realize: unsafe extern "C" fn(*mut c_void),
        gtk_widget_get_window: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        gdk_x11_window_get_xid: Option<unsafe extern "C" fn(*mut c_void) -> c_ulong>,
        g_signal_connect_data: unsafe extern "C" fn(
            *mut c_void,
            *const c_char,
            *const c_void,
            *mut c_void,
            *const c_void,
            c_int,
        ) -> c_ulong,
        webkit_web_context_new_ephemeral: unsafe extern "C" fn() -> *mut c_void,
        webkit_web_context_get_cookie_manager: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        webkit_cookie_manager_add_cookie: Option<
            unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void, *mut c_void),
        >,
        soup_cookie_new: Option<
            unsafe extern "C" fn(
                *const c_char,
                *const c_char,
                *const c_char,
                *const c_char,
                c_int,
            ) -> *mut c_void,
        >,
        soup_cookie_set_secure: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        webkit_user_content_manager_new: unsafe extern "C" fn() -> *mut c_void,
        webkit_user_script_new: unsafe extern "C" fn(
            *const c_char,
            c_int,
            c_int,
            *const *const c_char,
            *const *const c_char,
        ) -> *mut c_void,
        webkit_user_content_manager_add_script: unsafe extern "C" fn(*mut c_void, *mut c_void),
        webkit_web_view_new_with_context: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        webkit_web_view_get_user_content_manager:
            Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
        webkit_web_view_load_uri: unsafe extern "C" fn(*mut c_void, *const c_char),
        webkit_web_view_stop_loading: unsafe extern "C" fn(*mut c_void),
        webkit_web_view_get_settings: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        webkit_settings_set_user_agent: unsafe extern "C" fn(*mut c_void, *const c_char),
        webkit_navigation_policy_decision_get_navigation_action:
            Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
        webkit_navigation_action_get_request:
            Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
        webkit_uri_request_get_uri: Option<unsafe extern "C" fn(*mut c_void) -> *const c_char>,
        webkit_policy_decision_ignore: Option<unsafe extern "C" fn(*mut c_void)>,
        x_open_display: Option<unsafe extern "C" fn(*const c_char) -> *mut c_void>,
        x_reparent_window:
            Option<unsafe extern "C" fn(*mut c_void, c_ulong, c_ulong, c_int, c_int) -> c_int>,
        x_move_resize_window: Option<
            unsafe extern "C" fn(*mut c_void, c_ulong, c_int, c_int, c_uint, c_uint) -> c_int,
        >,
        x_flush: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    }

    static API: OnceLock<Api> = OnceLock::new();

    struct PageState {
        win: *mut c_void,
        view: *mut c_void,
        child_xid: Option<c_ulong>,
        _key_box: *mut String,
    }

    thread_local! {
        static LIVE: std::cell::RefCell<std::collections::HashMap<String, PageState>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
        static X_DPY: std::cell::Cell<*mut c_void> = const { std::cell::Cell::new(std::ptr::null_mut()) };
    }

    unsafe extern "C" fn on_decide_policy(
        _view: *mut c_void,
        decision: *mut c_void,
        decision_type: c_int,
        user_data: *mut c_void,
    ) -> c_int {
        // WEBKIT_POLICY_DECISION_TYPE_NAVIGATION_ACTION == 0.
        if decision_type != 0 {
            return 0;
        }
        let Some(api) = API.get() else { return 0 };
        let (Some(get_act), Some(get_req), Some(get_uri), Some(ignore)) = (
            api.webkit_navigation_policy_decision_get_navigation_action,
            api.webkit_navigation_action_get_request,
            api.webkit_uri_request_get_uri,
            api.webkit_policy_decision_ignore,
        ) else {
            return 0;
        };
        unsafe {
            let act = get_act(decision);
            if act.is_null() {
                return 0;
            }
            let req = get_req(act);
            if req.is_null() {
                return 0;
            }
            let uri_ptr = get_uri(req);
            if uri_ptr.is_null() {
                return 0;
            }
            let uri = CStr::from_ptr(uri_ptr).to_string_lossy();
            if uri.starts_with(&format!("{CLOSE_SCHEME}:")) {
                if !user_data.is_null() {
                    let key = &*(user_data as *const String);
                    crate::host::send_web_close(key.clone());
                }
                ignore(decision);
                return 1;
            }
            if let Some((scheme, _)) = uri.split_once(':')
                && !is_allowed_scheme(scheme)
            {
                ignore(decision);
                return 1;
            }
            0
        }
    }

    unsafe extern "C" fn drain_commands(_: *mut c_void) -> c_int {
        let Some(api) = API.get() else { return 0 };
        let Some(rx_lock) = QUEUE.get() else { return 0 };
        let Ok(rx) = rx_lock.lock() else { return 0 };
        while let Ok(cmd) = rx.try_recv() {
            unsafe {
                match cmd {
                    Cmd::Create {
                        key,
                        parent_xid,
                        rect,
                        props,
                    } => {
                        let ctx = (api.webkit_web_context_new_ephemeral)();
                        if !props.cookie_domain.is_empty()
                            && !props.cookies_file.is_empty()
                            && let (Some(add_c), Some(new_c)) =
                                (api.webkit_cookie_manager_add_cookie, api.soup_cookie_new)
                        {
                            let cm = (api.webkit_web_context_get_cookie_manager)(ctx);
                            if !cm.is_null()
                                && let (Ok(dom), Ok(path)) = (
                                    CString::new(props.cookie_domain.as_str()),
                                    CString::new("/"),
                                )
                            {
                                for (n, v) in parse_cookies(&props.cookies_file) {
                                    if let (Ok(cn), Ok(cv)) = (CString::new(n), CString::new(v)) {
                                        let c = new_c(
                                            cn.as_ptr(),
                                            cv.as_ptr(),
                                            dom.as_ptr(),
                                            path.as_ptr(),
                                            -1,
                                        );
                                        if !c.is_null() {
                                            if let Some(sec) = api.soup_cookie_set_secure {
                                                sec(c, 1);
                                            }
                                            add_c(
                                                cm,
                                                c,
                                                std::ptr::null_mut(),
                                                std::ptr::null_mut(),
                                                std::ptr::null_mut(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        let view = (api.webkit_web_view_new_with_context)(ctx);
                        if !props.close_on.is_empty() {
                            let ucm = api
                                .webkit_web_view_get_user_content_manager
                                .map(|f| f(view))
                                .unwrap_or_else(|| (api.webkit_user_content_manager_new)());
                            if !ucm.is_null()
                                && let Ok(js) = CString::new(close_script(&props.close_on))
                            {
                                let script = (api.webkit_user_script_new)(
                                    js.as_ptr(),
                                    1,
                                    0,
                                    std::ptr::null(),
                                    std::ptr::null(),
                                );
                                (api.webkit_user_content_manager_add_script)(ucm, script);
                            }
                        }
                        if !props.user_agent.is_empty()
                            && let Ok(ua) = CString::new(props.user_agent.as_str())
                        {
                            let settings = (api.webkit_web_view_get_settings)(view);
                            if !settings.is_null() {
                                (api.webkit_settings_set_user_agent)(settings, ua.as_ptr());
                            }
                        }
                        let key_box = Box::into_raw(Box::new(key.clone()));
                        (api.g_signal_connect_data)(
                            view,
                            c"decide-policy".as_ptr(),
                            on_decide_policy as *const c_void,
                            key_box as *mut c_void,
                            std::ptr::null(),
                            0,
                        );
                        let win = (api.gtk_window_new)(0);
                        (api.gtk_window_set_decorated)(win, 0);
                        (api.gtk_window_set_default_size)(win, rect.2, rect.3);
                        (api.gtk_container_add)(win, view);
                        (api.gtk_widget_realize)(win);
                        let mut child_xid = None;
                        if let (
                            Some(pxid),
                            Some(get_xid),
                            Some(reparent),
                            Some(move_resize),
                            Some(flush),
                        ) = (
                            parent_xid,
                            api.gdk_x11_window_get_xid,
                            api.x_reparent_window,
                            api.x_move_resize_window,
                            api.x_flush,
                        ) {
                            let gdk_win = (api.gtk_widget_get_window)(win);
                            if !gdk_win.is_null() {
                                let xid = get_xid(gdk_win);
                                let dpy = X_DPY.with(|d| {
                                    let mut p = d.get();
                                    if p.is_null()
                                        && let Some(open) = api.x_open_display
                                    {
                                        p = open(std::ptr::null());
                                        d.set(p);
                                    }
                                    p
                                });
                                if !dpy.is_null() && xid != 0 {
                                    reparent(dpy, xid, pxid, rect.0, rect.1);
                                    move_resize(
                                        dpy,
                                        xid,
                                        rect.0,
                                        rect.1,
                                        rect.2 as u32,
                                        rect.3 as u32,
                                    );
                                    flush(dpy);
                                    child_xid = Some(xid);
                                }
                            }
                        }
                        (api.gtk_widget_show_all)(win);
                        if !props.url.is_empty()
                            && let Ok(uri) = CString::new(props.url.as_str())
                        {
                            (api.webkit_web_view_load_uri)(view, uri.as_ptr());
                        }
                        LIVE.with(|live| {
                            live.borrow_mut().insert(
                                key,
                                PageState {
                                    win,
                                    view,
                                    child_xid,
                                    _key_box: key_box,
                                },
                            );
                        });
                    }
                    Cmd::Navigate { key, props } => {
                        LIVE.with(|live| {
                            if let Some(p) = live.borrow().get(&key) {
                                if !props.user_agent.is_empty()
                                    && let Ok(ua) = CString::new(props.user_agent.as_str())
                                {
                                    let settings = (api.webkit_web_view_get_settings)(p.view);
                                    if !settings.is_null() {
                                        (api.webkit_settings_set_user_agent)(settings, ua.as_ptr());
                                    }
                                }
                                if !props.url.is_empty()
                                    && let Ok(uri) = CString::new(props.url.as_str())
                                {
                                    (api.webkit_web_view_load_uri)(p.view, uri.as_ptr());
                                }
                            }
                        });
                    }
                    Cmd::Bounds { key, rect } => {
                        LIVE.with(|live| {
                            if let Some(p) = live.borrow().get(&key) {
                                (api.gtk_window_resize)(p.win, rect.2, rect.3);
                                if let (Some(xid), Some(move_resize), Some(flush)) =
                                    (p.child_xid, api.x_move_resize_window, api.x_flush)
                                {
                                    let dpy = X_DPY.get();
                                    if !dpy.is_null() {
                                        move_resize(
                                            dpy,
                                            xid,
                                            rect.0,
                                            rect.1,
                                            rect.2 as u32,
                                            rect.3 as u32,
                                        );
                                        flush(dpy);
                                    }
                                }
                            }
                        });
                    }
                    Cmd::Destroy { key } => {
                        LIVE.with(|live| {
                            if let Some(p) = live.borrow_mut().remove(&key) {
                                (api.webkit_web_view_stop_loading)(p.view);
                                (api.gtk_widget_destroy)(p.win);
                                drop(Box::from_raw(p._key_box));
                            }
                        });
                    }
                }
            }
        }
        0
    }

    fn runtime() -> Option<&'static GtkRuntime> {
        RUNTIME
            .get_or_init(|| unsafe {
                let flags = libc::RTLD_LAZY | libc::RTLD_LOCAL;
                let wk = libc::dlopen(c"libwebkit2gtk-4.1.so.0".as_ptr(), flags);
                let gtk = libc::dlopen(c"libgtk-3.so.0".as_ptr(), flags);
                let gdk = libc::dlopen(c"libgdk-3.so.0".as_ptr(), flags);
                let glib = libc::dlopen(c"libglib-2.0.so.0".as_ptr(), flags);
                let gobj = libc::dlopen(c"libgobject-2.0.so.0".as_ptr(), flags);
                if wk.is_null() || gtk.is_null() || glib.is_null() || gobj.is_null() {
                    eprintln!(
                        "bern: WebPage requires WebKitGTK (libwebkit2gtk-4.1.so.0 + libgtk-3.so.0)"
                    );
                    return None;
                }
                let soup = libc::dlopen(c"libsoup-3.0.so.0".as_ptr(), flags);
                let x11 = libc::dlopen(c"libX11.so.6".as_ptr(), flags);
                let wake = sym(glib, c"g_idle_add")?;
                let api = Api {
                    gtk_init: sym(gtk, c"gtk_init_check")?,
                    gtk_main: sym(gtk, c"gtk_main")?,
                    gtk_window_new: sym(gtk, c"gtk_window_new")?,
                    gtk_window_set_decorated: sym(gtk, c"gtk_window_set_decorated")?,
                    gtk_window_set_default_size: sym(gtk, c"gtk_window_set_default_size")?,
                    gtk_window_resize: sym(gtk, c"gtk_window_resize")?,
                    gtk_container_add: sym(gtk, c"gtk_container_add")?,
                    gtk_widget_show_all: sym(gtk, c"gtk_widget_show_all")?,
                    gtk_widget_destroy: sym(gtk, c"gtk_widget_destroy")?,
                    gtk_widget_realize: sym(gtk, c"gtk_widget_realize")?,
                    gtk_widget_get_window: sym(gtk, c"gtk_widget_get_window")?,
                    gdk_x11_window_get_xid: if gdk.is_null() {
                        None
                    } else {
                        sym(gdk, c"gdk_x11_window_get_xid")
                    },
                    g_signal_connect_data: sym(gobj, c"g_signal_connect_data")?,
                    webkit_web_context_new_ephemeral: sym(wk, c"webkit_web_context_new_ephemeral")?,
                    webkit_web_context_get_cookie_manager: sym(
                        wk,
                        c"webkit_web_context_get_cookie_manager",
                    )?,
                    webkit_cookie_manager_add_cookie: sym(wk, c"webkit_cookie_manager_add_cookie"),
                    soup_cookie_new: if soup.is_null() {
                        None
                    } else {
                        sym(soup, c"soup_cookie_new")
                    },
                    soup_cookie_set_secure: if soup.is_null() {
                        None
                    } else {
                        sym(soup, c"soup_cookie_set_secure")
                    },
                    webkit_user_content_manager_new: sym(wk, c"webkit_user_content_manager_new")?,
                    webkit_user_script_new: sym(wk, c"webkit_user_script_new")?,
                    webkit_user_content_manager_add_script: sym(
                        wk,
                        c"webkit_user_content_manager_add_script",
                    )?,
                    webkit_web_view_new_with_context: sym(wk, c"webkit_web_view_new_with_context")?,
                    webkit_web_view_get_user_content_manager: sym(
                        wk,
                        c"webkit_web_view_get_user_content_manager",
                    ),
                    webkit_web_view_load_uri: sym(wk, c"webkit_web_view_load_uri")?,
                    webkit_web_view_stop_loading: sym(wk, c"webkit_web_view_stop_loading")?,
                    webkit_web_view_get_settings: sym(wk, c"webkit_web_view_get_settings")?,
                    webkit_settings_set_user_agent: sym(wk, c"webkit_settings_set_user_agent")?,
                    webkit_navigation_policy_decision_get_navigation_action: sym(
                        wk,
                        c"webkit_navigation_policy_decision_get_navigation_action",
                    ),
                    webkit_navigation_action_get_request: sym(
                        wk,
                        c"webkit_navigation_action_get_request",
                    ),
                    webkit_uri_request_get_uri: sym(wk, c"webkit_uri_request_get_uri"),
                    webkit_policy_decision_ignore: sym(wk, c"webkit_policy_decision_ignore"),
                    x_open_display: if x11.is_null() {
                        None
                    } else {
                        sym(x11, c"XOpenDisplay")
                    },
                    x_reparent_window: if x11.is_null() {
                        None
                    } else {
                        sym(x11, c"XReparentWindow")
                    },
                    x_move_resize_window: if x11.is_null() {
                        None
                    } else {
                        sym(x11, c"XMoveResizeWindow")
                    },
                    x_flush: if x11.is_null() {
                        None
                    } else {
                        sym(x11, c"XFlush")
                    },
                };
                let _ = API.set(api);
                let (tx, rx) = std::sync::mpsc::channel();
                let _ = QUEUE.set(Mutex::new(rx));
                std::thread::spawn(move || {
                    let Some(api) = API.get() else { return };
                    let mut argc = 0;
                    let mut argv: *mut *mut c_char = std::ptr::null_mut();
                    if (api.gtk_init)(&mut argc, &mut argv) != 0 {
                        (api.gtk_main)();
                    }
                });
                Some(GtkRuntime { tx, wake })
            })
            .as_ref()
    }

    fn send(cmd: Cmd) {
        if let Some(rt) = runtime() {
            let _ = rt.tx.send(cmd);
            unsafe {
                (rt.wake)(drain_commands, std::ptr::null_mut());
            }
        }
    }

    pub(super) struct NativeWebView {
        key: String,
        last_rect: (i32, i32, i32, i32),
    }

    impl NativeWebView {
        #[allow(clippy::too_many_arguments)]
        pub(super) fn create(
            key: &str,
            window: &Window,
            x: f32,
            y: f32,
            w: f32,
            h: f32,
            scale: f32,
            props: &WebPageProps,
        ) -> Option<Self> {
            runtime()?;
            let parent_xid =
                HasWindowHandle::window_handle(window)
                    .ok()
                    .and_then(|h| match h.as_raw() {
                        RawWindowHandle::Xcb(x) => Some(x.window.get() as c_ulong),
                        RawWindowHandle::Xlib(x) => Some(x.window as c_ulong),
                        _ => None,
                    });
            let rect = (
                (x * scale).round() as i32,
                (y * scale).round() as i32,
                (w * scale).round().max(1.0) as i32,
                (h * scale).round().max(1.0) as i32,
            );
            send(Cmd::Create {
                key: key.to_string(),
                parent_xid,
                rect,
                props: props.clone(),
            });
            Some(Self {
                key: key.to_string(),
                last_rect: rect,
            })
        }

        pub(super) fn navigate(&mut self, props: &WebPageProps) {
            send(Cmd::Navigate {
                key: self.key.clone(),
                props: props.clone(),
            });
        }

        pub(super) fn set_bounds(
            &mut self,
            _window: &Window,
            x: f32,
            y: f32,
            w: f32,
            h: f32,
            scale: f32,
        ) {
            let rect = (
                (x * scale).round() as i32,
                (y * scale).round() as i32,
                (w * scale).round().max(1.0) as i32,
                (h * scale).round().max(1.0) as i32,
            );
            if rect != self.last_rect {
                self.last_rect = rect;
                send(Cmd::Bounds {
                    key: self.key.clone(),
                    rect,
                });
            }
        }

        pub(super) fn destroy(&mut self) {
            send(Cmd::Destroy {
                key: self.key.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_script_escapes_selector_as_json() {
        let js = close_script(r#"button[data-id="close"]"#);
        assert!(js.contains(r#"closest("button[data-id=\"close\"]")"#));
        assert!(js.contains("bern-web-page://close"));
    }

    #[test]
    fn scheme_filter_matches_web_schemes() {
        assert!(is_allowed_scheme("https"));
        assert!(is_allowed_scheme("http"));
        assert!(is_allowed_scheme("about"));
        assert!(!is_allowed_scheme("javascript"));
        assert!(!is_allowed_scheme("file"));
    }
}
