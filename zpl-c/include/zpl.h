#ifndef ZPL_H
#define ZPL_H
#include "zpl_config.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ABI 1. Compile and link against headers and library from the same release.
 * All functions use the platform C calling convention. Do not pack these structs.
 * All input pointers must be aligned and valid for the call. Mutable outputs must
 * not overlap inputs or other live library objects. A null input byte pointer is
 * allowed only with length zero; input bytes are copied or consumed during calls.
 * Optional options/limits/syntax pointers may be NULL to use Rust defaults.
 * Required NULL pointers and invalid indices return ZPL_INVALID_ARGUMENT.
 * Output handle slots must be writable and must not contain an unfreed handle.
 * Handle-producing calls set *out to NULL on failure. Other outputs are only
 * meaningful on success. Free each owned handle once using its matching free.
 * All free functions accept NULL. Never free a borrowed byte view yourself.
 * Independent calls and immutable handles may be used concurrently; do not free
 * a handle while another thread uses it. Errors are local to the calling thread.
 * Rust unwinding panics become ZPL_PANIC; process aborts/OOM cannot be caught.
 */
#define ZPL_ABI_VERSION 1u
#define ZPL_OK 0
#define ZPL_INVALID_ARGUMENT 1
#define ZPL_PARSE_ERROR 2
#define ZPL_RENDER_ERROR 3
#define ZPL_OUTPUT_ERROR 4
#define ZPL_PANIC 5
#define ZPL_PROFILE_ZD621 0u
#define ZPL_PROFILE_SPECIFICATION 1u
#define ZPL_PROFILE_ZQ610_PLUS 2u
#define ZPL_FORMAT_PNG 0u
#define ZPL_FORMAT_SVG 1u
#define ZPL_FORMAT_PDF 2u
/* Row-major grayscale, one byte per dot: 0 black, 255 white, no row padding. */
#define ZPL_FORMAT_GRAY8 3u
#define ZPL_ELEMENT_BEFORE_FIRST_COMMAND 0u
#define ZPL_ELEMENT_FORMAT_COMMAND 1u
#define ZPL_ELEMENT_CONTROL_COMMAND 2u
#define ZPL_ELEMENT_CONTROL_CHARACTER 3u

typedef struct ZplDocument ZplDocument;
typedef struct ZplScene ZplScene;
typedef struct ZplBuffer ZplBuffer;
typedef struct ZplParsed ZplParsed;
/* Bytes (including error strings and SVG) are NOT NUL-terminated. */
typedef struct { const uint8_t *data; size_t len; } ZplBytes;
/* offset == SIZE_MAX when the error has no input byte offset. */
typedef struct { int32_t status; size_t offset; ZplBytes message; } ZplError;
typedef struct { uint32_t width, height, dpi; } ZplSceneInfo;
typedef struct { uint32_t kind; size_t offset; ZplBytes data; } ZplElement;

uint32_t zpl_abi_version(void);
/* Static borrowed storage. */
ZplBytes zpl_library_version(void);
/* Borrowed until the next status-returning call on this thread (even success).
 * Accessors returning counts/views and free functions preserve the last error. */
ZplError zpl_last_error(void);
/* Initialize configurations before editing fields. Compatibility flags must be
 * 0 or 1. Optional presence flags distinguish None from Some(0). See the native
 * render::compatibility::Compatibility docs for each field's semantics. */
int32_t zpl_options_init(uint32_t profile, ZplOptions *out);
int32_t zpl_render_limits_init(ZplRenderLimits *out);
int32_t zpl_output_limits_init(ZplOutputLimits *out);
int32_t zpl_syntax_init(ZplSyntax *out);

int32_t zpl_render(const uint8_t *data, size_t len, const ZplOptions *options,
                   const ZplRenderLimits *limits, ZplDocument **out);
void zpl_document_free(ZplDocument *value);
/* Count accessors return zero for NULL. */
size_t zpl_document_label_count(const ZplDocument *value);
size_t zpl_document_warning_count(const ZplDocument *value);
/* Warning bytes are borrowed until the document is freed. */
int32_t zpl_document_warning(const ZplDocument *value, size_t index, ZplBytes *out);
/* Owned scene snapshot; survives freeing the document. */
int32_t zpl_document_label(const ZplDocument *value, size_t index, ZplScene **out);
void zpl_scene_free(ZplScene *value);
int32_t zpl_scene_info(const ZplScene *value, ZplSceneInfo *out);
/* Encoded buffers are independent of the source scene/document. */
int32_t zpl_scene_encode(const ZplScene *value, uint32_t format,
                         const ZplOutputLimits *limits, ZplBuffer **out);
int32_t zpl_document_pdf(const ZplDocument *value, const ZplOutputLimits *limits,
                         ZplBuffer **out);
/* Borrowed until buffer_free; NULL returns {NULL, 0}. */
ZplBytes zpl_buffer_bytes(const ZplBuffer *value);
void zpl_buffer_free(ZplBuffer *value);

/* Lossless framing, NOT validation or authorization. No renderer limits apply:
 * callers must bound parser input size. Source is copied into the result. */
int32_t zpl_parse(const uint8_t *data, size_t len, const ZplSyntax *syntax,
                  ZplParsed **out);
void zpl_parsed_free(ZplParsed *value);
size_t zpl_parsed_count(const ZplParsed *value);
/* Final syntax can be passed to the next complete-stream parse. */
int32_t zpl_parsed_syntax(const ZplParsed *value, ZplSyntax *out);
/* Element bytes are borrowed until parsed_free. */
int32_t zpl_parsed_element(const ZplParsed *value, size_t index, ZplElement *out);

#ifdef __cplusplus
}
#endif
#endif
