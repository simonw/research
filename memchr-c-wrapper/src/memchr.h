/*
 * memchr.h - High-performance byte and substring search functions
 *
 * This header provides SIMD-optimized search functions similar to the
 * BurntSushi/memchr Rust library.
 */

#ifndef MEMCHR_H
#define MEMCHR_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Single byte search - forward */
const uint8_t* memchr_find(uint8_t needle, const uint8_t* haystack, size_t len);
const uint8_t* memchr2_find(uint8_t n1, uint8_t n2, const uint8_t* haystack, size_t len);
const uint8_t* memchr3_find(uint8_t n1, uint8_t n2, uint8_t n3, const uint8_t* haystack, size_t len);

/* Single byte search - reverse */
const uint8_t* memrchr_find(uint8_t needle, const uint8_t* haystack, size_t len);
const uint8_t* memrchr2_find(uint8_t n1, uint8_t n2, const uint8_t* haystack, size_t len);
const uint8_t* memrchr3_find(uint8_t n1, uint8_t n2, uint8_t n3, const uint8_t* haystack, size_t len);

/* Substring search */
const uint8_t* memmem_find(const uint8_t* needle, size_t needle_len,
                           const uint8_t* haystack, size_t haystack_len);
const uint8_t* memmem_rfind(const uint8_t* needle, size_t needle_len,
                            const uint8_t* haystack, size_t haystack_len);

#ifdef __cplusplus
}
#endif

#endif /* MEMCHR_H */
