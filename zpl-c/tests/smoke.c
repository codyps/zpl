#include "zpl.h"
#include <assert.h>
#include <math.h>
#include <string.h>

int main(void) {
    assert(zpl_abi_version() == ZPL_ABI_VERSION);
    assert(zpl_library_version().len > 0);
    const uint8_t input[] = "^XA^FO1,1^GB4,4,1^FS^XZ^XA^XZ";
    ZplOptions options;
    ZplRenderLimits render_limits;
    ZplOutputLimits output_limits;
    ZplDocument *doc = NULL;
    ZplScene *scene = NULL;
    ZplBuffer *buffer = NULL;
    assert(zpl_options_init(ZPL_PROFILE_SPECIFICATION, &options) == ZPL_OK);
    assert(options.dpi == 203);
    options.width = 16;
    options.height = 12;
    assert(zpl_render_limits_init(&render_limits) == ZPL_OK);
    assert(zpl_output_limits_init(&output_limits) == ZPL_OK);
    assert(zpl_render(input, sizeof(input)-1, &options, &render_limits, &doc) == ZPL_OK);
    assert(zpl_document_label_count(doc) == 2);
    assert(zpl_document_warning_count(doc) == 0);
    assert(zpl_document_pdf(doc, &output_limits, &buffer) == ZPL_OK);
    assert(memcmp(zpl_buffer_bytes(buffer).data, "%PDF-", 5) == 0);
    zpl_buffer_free(buffer);
    assert(zpl_document_label(doc, 0, &scene) == ZPL_OK);
    zpl_document_free(doc);
    ZplSceneInfo info;
    assert(zpl_scene_info(scene, &info) == ZPL_OK);
    assert(info.width == 16 && info.height == 12 && info.dpi == 203);
    for (uint32_t format = 0; format < 4; ++format) {
        assert(zpl_scene_encode(scene, format, &output_limits, &buffer) == ZPL_OK);
        ZplBytes bytes = zpl_buffer_bytes(buffer);
        assert(bytes.len > 0);
        if (format == ZPL_FORMAT_PNG) assert(memcmp(bytes.data, "\x89PNG\r\n\x1a\n", 8) == 0);
        if (format == ZPL_FORMAT_PDF) assert(memcmp(bytes.data, "%PDF-", 5) == 0);
        if (format == ZPL_FORMAT_GRAY8) {
            assert(bytes.len == 16*12);
            assert(bytes.data[0] == 255 && bytes.data[17] == 0);
        }
        zpl_buffer_free(buffer);
    }
    assert(zpl_scene_encode(scene, 999, NULL, &buffer) == ZPL_INVALID_ARGUMENT);
    assert(buffer == NULL);
    output_limits.pixels = 1;
    assert(zpl_scene_encode(scene, ZPL_FORMAT_GRAY8, &output_limits, &buffer) == ZPL_OUTPUT_ERROR);
    assert(buffer == NULL);
    ZplError error = zpl_last_error();
    assert(error.status == ZPL_OUTPUT_ERROR && error.offset == SIZE_MAX && error.message.len > 0);
    zpl_scene_free(scene);
    /* An optional Some(0) is distinct from None and is rejected by the renderer. */
    options.compatibility.preview_width_quantum.present = 1;
    options.compatibility.preview_width_quantum.value = 0;
    assert(zpl_render(input, sizeof(input)-1, &options, NULL, &doc) == ZPL_RENDER_ERROR);
    assert(doc == NULL);
    options.compatibility.preview_width_quantum.present = 0;
    options.compatibility.code128_above_text_keeps_bar_origin = 2; /* Last field checks full layout. */
    assert(zpl_render(input, sizeof(input)-1, &options, NULL, &doc) == ZPL_INVALID_ARGUMENT);
    options.compatibility.code128_above_text_keeps_bar_origin = 0;
    /* The new caption flag must reach native boolean validation. */
    assert(options.compatibility.barcode_implicit_caption_uses_resident_font == 0);
    options.compatibility.barcode_implicit_caption_uses_resident_font = 2;
    assert(zpl_render(input, sizeof(input)-1, &options, NULL, &doc) == ZPL_INVALID_ARGUMENT);
    assert(doc == NULL);
    options.compatibility.barcode_implicit_caption_uses_resident_font = 0;
    render_limits.number_abs = NAN;
    assert(zpl_render(input, sizeof(input)-1, &options, &render_limits, &doc) == ZPL_INVALID_ARGUMENT);
    assert(zpl_render(NULL, 1, NULL, NULL, &doc) == ZPL_INVALID_ARGUMENT);
    assert(zpl_render(NULL, 0, NULL, NULL, NULL) == ZPL_INVALID_ARGUMENT);
    assert(zpl_options_init(999, &options) == ZPL_INVALID_ARGUMENT);

    uint8_t binary[] = "prefix^XA^FDa\0b^FS^GFB,4,4,4,^~\0\xff^FS^CC!!XZ";
    uint8_t original[sizeof(binary)];
    memcpy(original, binary, sizeof(binary));
    ZplParsed *parsed = NULL;
    ZplSyntax syntax;
    assert(zpl_syntax_init(&syntax) == ZPL_OK);
    assert(syntax.format_prefix == '^' && syntax.control_prefix == '~' && syntax.delimiter == ',');
    assert(zpl_parse(binary, sizeof(binary)-1, &syntax, &parsed) == ZPL_OK);
    memset(binary, 0, sizeof(binary));
    size_t offset = 0;
    for (size_t i = 0; i < zpl_parsed_count(parsed); ++i) {
        ZplElement element;
        assert(zpl_parsed_element(parsed, i, &element) == ZPL_OK);
        assert(element.offset == offset);
        assert(memcmp(element.data.data, original + offset, element.data.len) == 0);
        if (i == 0) assert(element.kind == ZPL_ELEMENT_BEFORE_FIRST_COMMAND);
        offset += element.data.len;
    }
    assert(offset == sizeof(binary)-1);
    assert(zpl_parsed_syntax(parsed, &syntax) == ZPL_OK && syntax.format_prefix == '!');
    zpl_parsed_free(parsed);
    assert(zpl_parse((const uint8_t *)"!XA!XZ", 6, &syntax, &parsed) == ZPL_OK);
    assert(zpl_parsed_count(parsed) == 2);
    zpl_parsed_free(parsed);
    assert(zpl_parse((const uint8_t *)"^XA^", 4, NULL, &parsed) == ZPL_PARSE_ERROR);
    assert(parsed == NULL && zpl_last_error().offset == 3);
    assert(zpl_parse(NULL, 0, NULL, &parsed) == ZPL_OK);
    assert(zpl_parsed_count(parsed) == 0 && zpl_last_error().status == ZPL_OK);
    zpl_parsed_free(parsed);
    zpl_document_free(NULL);
    zpl_scene_free(NULL);
    zpl_buffer_free(NULL);
    zpl_parsed_free(NULL);
    return 0;
}
