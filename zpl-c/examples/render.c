/* Build instructions: ../README.md. Render locally; no printer access. */
#include "zpl.h"
#include <stdio.h>

int main(void) {
    const uint8_t source[] = "^XA^PW320^LL160^FO10,10^A0N,24,24^FDHello, C!^FS^XZ";
    ZplDocument *document = NULL;
    ZplScene *scene = NULL;
    ZplBuffer *png = NULL;
    FILE *file = NULL;
    int result = 1;
    if (zpl_render(source, sizeof(source) - 1, NULL, NULL, &document) != ZPL_OK ||
        zpl_document_label(document, 0, &scene) != ZPL_OK ||
        zpl_scene_encode(scene, ZPL_FORMAT_PNG, NULL, &png) != ZPL_OK) {
        ZplError error = zpl_last_error();
        fwrite(error.message.data, 1, error.message.len, stderr);
        fputc('\n', stderr);
        goto cleanup;
    }
    file = fopen("label.png", "wb");
    if (file == NULL) {
        perror("label.png");
        goto cleanup;
    }
    ZplBytes bytes = zpl_buffer_bytes(png);
    result = fwrite(bytes.data, 1, bytes.len, file) == bytes.len ? 0 : 1;
    if (fclose(file) != 0) result = 1;
cleanup:
    zpl_buffer_free(png);
    zpl_scene_free(scene);
    zpl_document_free(document);
    return result;
}
