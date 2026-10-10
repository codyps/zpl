/* Private prototype ABI. Ownership and wire format: bridge/src/lib.rs. */
#include <stdint.h>
#include <stddef.h>
uint32_t zp_abi(void);
void *zp_alloc(size_t);
void zp_dealloc(void *, size_t);
void *zp_render(const void *, size_t, uint32_t, uint32_t, uint32_t, uint32_t);
const void *zp_result_data(const void *);
size_t zp_result_len(const void *);
void zp_result_free(void *);
