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

BernApp *bern_app_start(uint64_t fingerprint, BernCallbacks callbacks, BernBuffer *error);
void bern_app_stop(BernApp *app);
BernBuffer bern_app_snapshot(BernApp *app);
BernBuffer bern_app_engine_id(BernApp *app);
BernBuffer bern_app_channel(BernApp *app);
void bern_app_dispatch(BernApp *app, uint32_t command, const uint8_t *params, size_t len, uint64_t token);
BernBuffer bern_window_opened(BernApp *app, uint32_t kind);
void bern_window_closed(BernApp *app, uint32_t kind, uint64_t generation);
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
void bern_buffer_free(BernBuffer buffer);

#endif
