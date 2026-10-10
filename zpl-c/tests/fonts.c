/* The actual C callback ABI, with borrowed context and persistent glyph storage.
 * Bitmap/selection contracts: zpl::fonts::BitmapFont and Fonts, not printer evidence. */
#include "zpl.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const uint8_t bitmap[] = {0x40, 0xa0, 0xe0};
static int32_t glyph(void *context, uint32_t codepoint, ZplGlyph *out, ZplBytes *error) {
    (void)error;
    assert(context == bitmap);
    if (codepoint != 'A') return ZPL_GLYPH_MISSING;
    out->advance = 8;
    out->left = 0;
    out->top = -3;
    out->width = 3;
    out->height = 3;
    out->bitmap.data = (const uint8_t *)context;
    out->bitmap.len = sizeof(bitmap);
    return ZPL_GLYPH_FOUND;
}
static ZplDocument *render(const char *source, const ZplOptions *options, const ZplFonts *fonts) {
    ZplDocument *doc = NULL;
    int32_t status = zpl_render_with_fonts((const uint8_t *)source, strlen(source), options, fonts, NULL, &doc);
    if (status != ZPL_OK) {
        ZplError error = zpl_last_error();
        fwrite(error.message.data, 1, error.message.len, stderr);
    }
    assert(status == ZPL_OK);
    return doc;
}
int main(void) {
    ZplFonts *fonts = NULL;
    ZplOptions options;
    assert(zpl_fonts_new(&fonts) == ZPL_OK);
    assert(zpl_options_init(ZPL_PROFILE_SPECIFICATION, &options) == ZPL_OK);
    options.width = 64;
    options.height = 48;
    ZplBitmapProvider provider = {{8, 12, 9.0}, (void *)bitmap, glyph};
    assert(zpl_fonts_insert_bitmap(fonts, 'Z', &provider) == ZPL_OK);
    const uint8_t name[] = "probe.fnt";
    assert(zpl_fonts_insert_named_bitmap(fonts, name, sizeof(name)-1, &provider) == ZPL_OK);
    /* Invalid metrics and null callbacks do not erase the registered face. */
    provider.metrics.width = 0;
    assert(zpl_fonts_insert_bitmap(fonts, 'Z', &provider) == ZPL_FONT_ERROR);
    provider.metrics.width = 8;
    provider.glyph = NULL;
    assert(zpl_fonts_insert_bitmap(fonts, 'Z', &provider) == ZPL_INVALID_ARGUMENT);
    assert(zpl_fonts_insert_bitmap(NULL, 'Z', &provider) == ZPL_INVALID_ARGUMENT);
    assert(zpl_fonts_insert_bitmap(fonts, 'Z', NULL) == ZPL_INVALID_ARGUMENT);
    ZplDocument *direct = render("^XA^FO2,3^AZN,12,8^FDA^FS^XZ", &options, fonts);
    ZplDocument *named = render("^CWY,R:PROBE.FNT^XA^FO2,3^AYN,12,8^FDA^FS^XZ", &options, fonts);
    ZplDocument *missing = NULL;
    const uint8_t absent[] = "^XA^AZN,12,8^FDB^FS^XZ";
    assert(zpl_render_with_fonts(absent, sizeof(absent)-1, &options, fonts, NULL, &missing) == ZPL_RENDER_ERROR);
    assert(missing == NULL && zpl_last_error().offset < sizeof(absent)-1);
    zpl_fonts_free(fonts);
    ZplBuffer *a = NULL, *b = NULL;
    assert(zpl_document_pdf(direct, NULL, &a) == ZPL_OK);
    assert(zpl_document_pdf(named, NULL, &b) == ZPL_OK);
    ZplBytes first = zpl_buffer_bytes(a), second = zpl_buffer_bytes(b);
    assert(first.len == second.len && memcmp(first.data, second.data, first.len) == 0);
    zpl_buffer_free(a);
    zpl_buffer_free(b);
    zpl_document_free(direct);
    zpl_document_free(named);

    FILE *file = fopen("zpl/tests/fixtures/truetype-regression/font-probes-20261002/probe.ttf", "rb");
    assert(file != NULL);
    assert(fseek(file, 0, SEEK_END) == 0);
    long size = ftell(file);
    assert(size > 0);
    rewind(file);
    uint8_t *data = (uint8_t *)malloc((size_t)size);
    assert(data != NULL);
    assert(fread(data, 1, (size_t)size, file) == (size_t)size);
    assert(fclose(file) == 0);
    assert(zpl_fonts_new(&fonts) == ZPL_OK);
    const uint8_t ttf_name[] = "probe.ttf";
    assert(zpl_fonts_insert_truetype(fonts, 'Z', data, (size_t)size, ZPL_HINTING_NATIVE) == ZPL_OK);
    assert(zpl_fonts_insert_named_truetype(fonts, ttf_name, sizeof(ttf_name)-1, data, (size_t)size, ZPL_HINTING_NATIVE) == ZPL_OK);
    memset(data, 0, (size_t)size);
    assert(zpl_fonts_insert_truetype(fonts, 'Z', data, (size_t)size, ZPL_HINTING_NATIVE) == ZPL_FONT_ERROR);
    assert(zpl_fonts_insert_truetype(fonts, 'Z', data, (size_t)size, 99) == ZPL_INVALID_ARGUMENT);
    free(data);
    direct = render("^XA^FO2,3^AZN,24,20^FD!^FS^XZ", &options, fonts);
    named = render("^XA^FO2,3^A@N,24,20,probe.ttf^FD!^FS^XZ", &options, fonts);
    zpl_fonts_free(fonts);
    assert(zpl_document_pdf(direct, NULL, &a) == ZPL_OK);
    assert(zpl_document_pdf(named, NULL, &b) == ZPL_OK);
    first = zpl_buffer_bytes(a);
    second = zpl_buffer_bytes(b);
    assert(first.len == second.len && memcmp(first.data, second.data, first.len) == 0);
    zpl_buffer_free(a);
    zpl_buffer_free(b);
    zpl_document_free(direct);
    zpl_document_free(named);
    zpl_fonts_free(NULL);
    return 0;
}
