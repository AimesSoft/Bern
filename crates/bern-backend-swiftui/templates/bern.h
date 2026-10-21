// Bern in-process bridge. Generated build input; do not edit.
//
// Values cross this boundary in Bern's compact binary layout, typed on both
// sides by generated code. Callbacks may run on any thread.

#ifndef BERN_H
#define BERN_H

#include <stddef.h>
#include <stdint.h>

typedef struct BernApp BernApp;

typedef struct {
    uint8_t *ptr;
    size_t len;
    size_t capacity;
} BernBuffer;

typedef struct {
    void *context;
    void (*on_state)(void *context, const uint8_t *bytes, size_t len);
    void (*on_reply)(void *context, uint64_t token, const uint8_t *bytes, size_t len);
    void (*on_session_request)(void *context, uint64_t request, const uint8_t *bytes, size_t len);
    void (*on_media_request)(void *context, const uint8_t *bytes, size_t len);
    void (*on_media_state)(void *context, const uint8_t *bytes, size_t len);
    void (*on_query)(void *context, uint64_t subscription, const uint8_t *bytes, size_t len);
} BernCallbacks;

// `resources`: UTF-8 path of the bundle directory holding the declared
// resource files, or NULL with length 0.
BernApp *bern_app_start(uint64_t fingerprint, const uint8_t *resources, size_t resources_len, BernCallbacks callbacks, BernBuffer *error);
// iOS only, before `bern_app_start`: whether windows other than settings
// are native windows, as where the app can open several (iPad).
void bern_app_native_windows(uint8_t native);
void bern_app_stop(BernApp *app);
BernBuffer bern_app_snapshot(BernApp *app);
BernBuffer bern_app_engine_id(BernApp *app);
BernBuffer bern_app_channel(BernApp *app);
void bern_app_dispatch(BernApp *app, uint32_t command, const uint8_t *params, size_t len, uint64_t token);
BernBuffer bern_window_opened(BernApp *app, uint32_t kind);
void bern_window_closed(BernApp *app, uint32_t kind, uint64_t generation);
// Presentation facts of a window instance; see `bern_window_facts` in bern-ffi.
uint64_t bern_window_facts(BernApp *app, uint32_t kind, uint64_t generation, uint8_t source, uint8_t visibility, uint8_t size, double x, double y, double width, double height, uint8_t focused, uint32_t actions);
BernBuffer bern_window_request_close(BernApp *app, uint32_t kind, uint64_t generation, uint8_t needs_confirmation, uint8_t can_save);
BernBuffer bern_window_confirm_close(BernApp *app, uint32_t kind, uint64_t generation, const uint8_t *bytes, size_t len);
BernBuffer bern_session_set(BernApp *app, uint32_t kind, uint64_t generation, uint32_t session, uint32_t field, const uint8_t *value, size_t len);
void bern_session_complete(BernApp *app, uint64_t request, const uint8_t *bytes, size_t len);
BernBuffer bern_query_open(BernApp *app, uint32_t kind, uint64_t generation, uint32_t query);
BernBuffer bern_query_request(BernApp *app, uint64_t subscription, const uint8_t *params, size_t len);
void bern_query_close(BernApp *app, uint64_t subscription);
BernBuffer bern_media_executor_attached(BernApp *app, const uint8_t *bytes, size_t len);
void bern_media_executor_detached(BernApp *app);
BernBuffer bern_media_snapshot(BernApp *app);
void bern_media_dispatch(BernApp *app, uint32_t command, const uint8_t *params, size_t len, uint64_t token);
BernBuffer bern_media_create(BernApp *app, const uint8_t *bytes, size_t len);
BernBuffer bern_media_bind(BernApp *app, const uint8_t *bytes, size_t len);
void bern_media_release(BernApp *app, const uint8_t *bytes, size_t len);
void bern_media_view(BernApp *app, const uint8_t *bytes, size_t len);
void bern_media_complete(BernApp *app, const uint8_t *bytes, size_t len);
void bern_media_event(BernApp *app, const uint8_t *bytes, size_t len);
// The application's media kernel; see `bern_media_kernel_attach` in bern-ffi.
BernBuffer bern_media_kernel_attach(BernApp *app, const uint8_t *bytes, size_t len);
void bern_media_surface_attach(BernApp *app, const uint8_t *bytes, size_t len);
void bern_media_surface_resize(BernApp *app, const uint8_t *bytes, size_t len);
void bern_media_surface_detach(BernApp *app, const uint8_t *bytes, size_t len);
void bern_media_surface_frame(BernApp *app, const uint8_t *view, size_t len, double now, double target);
void bern_buffer_free(BernBuffer buffer);

#endif
