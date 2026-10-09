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
} BernCallbacks;

BernApp *bern_app_start(uint64_t fingerprint, BernCallbacks callbacks, BernBuffer *error);
void bern_app_stop(BernApp *app);
BernBuffer bern_app_snapshot(BernApp *app);
BernBuffer bern_app_engine_id(BernApp *app);
BernBuffer bern_app_channel(BernApp *app);
void bern_app_dispatch(BernApp *app, uint32_t command, const uint8_t *params, size_t len, uint64_t token);
BernBuffer bern_window_opened(BernApp *app, uint32_t kind);
void bern_window_closed(BernApp *app, uint32_t kind, uint64_t generation);
BernBuffer bern_session_set(BernApp *app, uint32_t kind, uint64_t generation, uint32_t session, uint32_t field, const uint8_t *value, size_t len);
void bern_session_complete(BernApp *app, uint64_t request, const uint8_t *bytes, size_t len);
void bern_buffer_free(BernBuffer buffer);

#endif
