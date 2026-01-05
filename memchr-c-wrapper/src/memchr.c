/*
 * memchr.c - High-performance byte and substring search functions
 *
 * Implementation uses SIMD optimizations where available:
 * - SSE2/AVX2 on x86-64
 * - NEON on ARM64
 * - Fallback scalar implementation for other platforms
 */

#include "memchr.h"
#include <string.h>

/* Detect platform and SIMD capabilities */
#if defined(__x86_64__) || defined(_M_X64)
    #define USE_SSE2 1
    #include <emmintrin.h>
    #if defined(__AVX2__)
        #define USE_AVX2 1
        #include <immintrin.h>
    #endif
#elif defined(__aarch64__) || defined(_M_ARM64)
    #define USE_NEON 1
    #include <arm_neon.h>
#endif

/*
 * Single byte search - forward
 * Uses libc memchr which is highly optimized on most platforms
 */
const uint8_t* memchr_find(uint8_t needle, const uint8_t* haystack, size_t len) {
    return (const uint8_t*)memchr(haystack, needle, len);
}

/*
 * Single byte search - reverse
 * Uses libc memrchr if available, otherwise custom implementation
 */
const uint8_t* memrchr_find(uint8_t needle, const uint8_t* haystack, size_t len) {
#if defined(_GNU_SOURCE) || defined(__GLIBC__)
    return (const uint8_t*)memrchr(haystack, needle, len);
#else
    /* Fallback implementation */
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack + len;

    /* Handle unaligned tail bytes first */
    while (ptr > haystack && ((uintptr_t)ptr & 7) != 0) {
        ptr--;
        if (*ptr == needle) return ptr;
    }

    /* Process 8 bytes at a time for aligned portion */
    while (ptr >= haystack + 8) {
        ptr -= 8;
        for (int i = 7; i >= 0; i--) {
            if (ptr[i] == needle) return ptr + i;
        }
    }

    /* Handle remaining bytes */
    while (ptr > haystack) {
        ptr--;
        if (*ptr == needle) return ptr;
    }

    return NULL;
#endif
}

#ifdef USE_SSE2
/*
 * SSE2 implementation for memchr2
 * Searches for first occurrence of either of two bytes
 */
static inline const uint8_t* memchr2_sse2(uint8_t n1, uint8_t n2,
                                           const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + len;

    /* Handle unaligned prefix */
    while (ptr < end && ((uintptr_t)ptr & 15) != 0) {
        if (*ptr == n1 || *ptr == n2) return ptr;
        ptr++;
    }

    if (ptr >= end) return NULL;

    /* SSE2 vectorized search */
    __m128i needle1 = _mm_set1_epi8(n1);
    __m128i needle2 = _mm_set1_epi8(n2);

    while (ptr + 16 <= end) {
        __m128i chunk = _mm_load_si128((const __m128i*)ptr);
        __m128i cmp1 = _mm_cmpeq_epi8(chunk, needle1);
        __m128i cmp2 = _mm_cmpeq_epi8(chunk, needle2);
        __m128i result = _mm_or_si128(cmp1, cmp2);
        int mask = _mm_movemask_epi8(result);

        if (mask != 0) {
            int idx = __builtin_ctz(mask);
            return ptr + idx;
        }
        ptr += 16;
    }

    /* Handle remaining bytes */
    while (ptr < end) {
        if (*ptr == n1 || *ptr == n2) return ptr;
        ptr++;
    }

    return NULL;
}

/*
 * SSE2 implementation for memchr3
 * Searches for first occurrence of any of three bytes
 */
static inline const uint8_t* memchr3_sse2(uint8_t n1, uint8_t n2, uint8_t n3,
                                           const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + len;

    /* Handle unaligned prefix */
    while (ptr < end && ((uintptr_t)ptr & 15) != 0) {
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
        ptr++;
    }

    if (ptr >= end) return NULL;

    /* SSE2 vectorized search */
    __m128i needle1 = _mm_set1_epi8(n1);
    __m128i needle2 = _mm_set1_epi8(n2);
    __m128i needle3 = _mm_set1_epi8(n3);

    while (ptr + 16 <= end) {
        __m128i chunk = _mm_load_si128((const __m128i*)ptr);
        __m128i cmp1 = _mm_cmpeq_epi8(chunk, needle1);
        __m128i cmp2 = _mm_cmpeq_epi8(chunk, needle2);
        __m128i cmp3 = _mm_cmpeq_epi8(chunk, needle3);
        __m128i result = _mm_or_si128(_mm_or_si128(cmp1, cmp2), cmp3);
        int mask = _mm_movemask_epi8(result);

        if (mask != 0) {
            int idx = __builtin_ctz(mask);
            return ptr + idx;
        }
        ptr += 16;
    }

    /* Handle remaining bytes */
    while (ptr < end) {
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
        ptr++;
    }

    return NULL;
}

/*
 * SSE2 implementation for memrchr2
 */
static inline const uint8_t* memrchr2_sse2(uint8_t n1, uint8_t n2,
                                            const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* end = haystack + len;
    const uint8_t* ptr = end;

    /* Handle unaligned suffix */
    while (ptr > haystack && ((uintptr_t)ptr & 15) != 0) {
        ptr--;
        if (*ptr == n1 || *ptr == n2) return ptr;
    }

    if (ptr <= haystack) return NULL;

    /* SSE2 vectorized search */
    __m128i needle1 = _mm_set1_epi8(n1);
    __m128i needle2 = _mm_set1_epi8(n2);

    while (ptr >= haystack + 16) {
        ptr -= 16;
        __m128i chunk = _mm_load_si128((const __m128i*)ptr);
        __m128i cmp1 = _mm_cmpeq_epi8(chunk, needle1);
        __m128i cmp2 = _mm_cmpeq_epi8(chunk, needle2);
        __m128i result = _mm_or_si128(cmp1, cmp2);
        int mask = _mm_movemask_epi8(result);

        if (mask != 0) {
            int idx = 15 - __builtin_clz(mask) + 16;
            return ptr + idx;
        }
    }

    /* Handle remaining bytes */
    while (ptr > haystack) {
        ptr--;
        if (*ptr == n1 || *ptr == n2) return ptr;
    }

    return NULL;
}

/*
 * SSE2 implementation for memrchr3
 */
static inline const uint8_t* memrchr3_sse2(uint8_t n1, uint8_t n2, uint8_t n3,
                                            const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* end = haystack + len;
    const uint8_t* ptr = end;

    /* Handle unaligned suffix */
    while (ptr > haystack && ((uintptr_t)ptr & 15) != 0) {
        ptr--;
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
    }

    if (ptr <= haystack) return NULL;

    /* SSE2 vectorized search */
    __m128i needle1 = _mm_set1_epi8(n1);
    __m128i needle2 = _mm_set1_epi8(n2);
    __m128i needle3 = _mm_set1_epi8(n3);

    while (ptr >= haystack + 16) {
        ptr -= 16;
        __m128i chunk = _mm_load_si128((const __m128i*)ptr);
        __m128i cmp1 = _mm_cmpeq_epi8(chunk, needle1);
        __m128i cmp2 = _mm_cmpeq_epi8(chunk, needle2);
        __m128i cmp3 = _mm_cmpeq_epi8(chunk, needle3);
        __m128i result = _mm_or_si128(_mm_or_si128(cmp1, cmp2), cmp3);
        int mask = _mm_movemask_epi8(result);

        if (mask != 0) {
            int idx = 15 - __builtin_clz(mask) + 16;
            return ptr + idx;
        }
    }

    /* Handle remaining bytes */
    while (ptr > haystack) {
        ptr--;
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
    }

    return NULL;
}

#endif /* USE_SSE2 */

#ifdef USE_NEON
/*
 * NEON implementation for memchr2
 */
static inline const uint8_t* memchr2_neon(uint8_t n1, uint8_t n2,
                                           const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + len;

    /* Handle unaligned prefix */
    while (ptr < end && ((uintptr_t)ptr & 15) != 0) {
        if (*ptr == n1 || *ptr == n2) return ptr;
        ptr++;
    }

    if (ptr >= end) return NULL;

    /* NEON vectorized search */
    uint8x16_t needle1 = vdupq_n_u8(n1);
    uint8x16_t needle2 = vdupq_n_u8(n2);

    while (ptr + 16 <= end) {
        uint8x16_t chunk = vld1q_u8(ptr);
        uint8x16_t cmp1 = vceqq_u8(chunk, needle1);
        uint8x16_t cmp2 = vceqq_u8(chunk, needle2);
        uint8x16_t result = vorrq_u8(cmp1, cmp2);

        uint64x2_t result64 = vreinterpretq_u64_u8(result);
        if (vgetq_lane_u64(result64, 0) || vgetq_lane_u64(result64, 1)) {
            for (int i = 0; i < 16 && ptr + i < end; i++) {
                if (ptr[i] == n1 || ptr[i] == n2) return ptr + i;
            }
        }
        ptr += 16;
    }

    /* Handle remaining bytes */
    while (ptr < end) {
        if (*ptr == n1 || *ptr == n2) return ptr;
        ptr++;
    }

    return NULL;
}

/*
 * NEON implementation for memchr3
 */
static inline const uint8_t* memchr3_neon(uint8_t n1, uint8_t n2, uint8_t n3,
                                           const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + len;

    /* Handle unaligned prefix */
    while (ptr < end && ((uintptr_t)ptr & 15) != 0) {
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
        ptr++;
    }

    if (ptr >= end) return NULL;

    /* NEON vectorized search */
    uint8x16_t needle1 = vdupq_n_u8(n1);
    uint8x16_t needle2 = vdupq_n_u8(n2);
    uint8x16_t needle3 = vdupq_n_u8(n3);

    while (ptr + 16 <= end) {
        uint8x16_t chunk = vld1q_u8(ptr);
        uint8x16_t cmp1 = vceqq_u8(chunk, needle1);
        uint8x16_t cmp2 = vceqq_u8(chunk, needle2);
        uint8x16_t cmp3 = vceqq_u8(chunk, needle3);
        uint8x16_t result = vorrq_u8(vorrq_u8(cmp1, cmp2), cmp3);

        uint64x2_t result64 = vreinterpretq_u64_u8(result);
        if (vgetq_lane_u64(result64, 0) || vgetq_lane_u64(result64, 1)) {
            for (int i = 0; i < 16 && ptr + i < end; i++) {
                if (ptr[i] == n1 || ptr[i] == n2 || ptr[i] == n3) return ptr + i;
            }
        }
        ptr += 16;
    }

    /* Handle remaining bytes */
    while (ptr < end) {
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
        ptr++;
    }

    return NULL;
}

/*
 * NEON implementation for memrchr2
 */
static inline const uint8_t* memrchr2_neon(uint8_t n1, uint8_t n2,
                                            const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* end = haystack + len;
    const uint8_t* ptr = end;

    /* Scalar fallback for reverse - NEON is less efficient for reverse */
    while (ptr > haystack) {
        ptr--;
        if (*ptr == n1 || *ptr == n2) return ptr;
    }

    return NULL;
}

/*
 * NEON implementation for memrchr3
 */
static inline const uint8_t* memrchr3_neon(uint8_t n1, uint8_t n2, uint8_t n3,
                                            const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* end = haystack + len;
    const uint8_t* ptr = end;

    /* Scalar fallback for reverse */
    while (ptr > haystack) {
        ptr--;
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
    }

    return NULL;
}

#endif /* USE_NEON */

/*
 * Scalar fallback implementation for memchr2
 */
static inline const uint8_t* memchr2_scalar(uint8_t n1, uint8_t n2,
                                             const uint8_t* haystack, size_t len) {
    const uint8_t* end = haystack + len;

    for (const uint8_t* ptr = haystack; ptr < end; ptr++) {
        if (*ptr == n1 || *ptr == n2) return ptr;
    }

    return NULL;
}

/*
 * Scalar fallback implementation for memchr3
 */
static inline const uint8_t* memchr3_scalar(uint8_t n1, uint8_t n2, uint8_t n3,
                                             const uint8_t* haystack, size_t len) {
    const uint8_t* end = haystack + len;

    for (const uint8_t* ptr = haystack; ptr < end; ptr++) {
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
    }

    return NULL;
}

/*
 * Scalar fallback implementation for memrchr2
 */
static inline const uint8_t* memrchr2_scalar(uint8_t n1, uint8_t n2,
                                              const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack + len;

    while (ptr > haystack) {
        ptr--;
        if (*ptr == n1 || *ptr == n2) return ptr;
    }

    return NULL;
}

/*
 * Scalar fallback implementation for memrchr3
 */
static inline const uint8_t* memrchr3_scalar(uint8_t n1, uint8_t n2, uint8_t n3,
                                              const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack + len;

    while (ptr > haystack) {
        ptr--;
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
    }

    return NULL;
}

/*
 * Public memchr2 function - dispatches to best implementation
 */
const uint8_t* memchr2_find(uint8_t n1, uint8_t n2, const uint8_t* haystack, size_t len) {
#ifdef USE_SSE2
    return memchr2_sse2(n1, n2, haystack, len);
#elif defined(USE_NEON)
    return memchr2_neon(n1, n2, haystack, len);
#else
    return memchr2_scalar(n1, n2, haystack, len);
#endif
}

/*
 * Public memchr3 function - dispatches to best implementation
 */
const uint8_t* memchr3_find(uint8_t n1, uint8_t n2, uint8_t n3,
                            const uint8_t* haystack, size_t len) {
#ifdef USE_SSE2
    return memchr3_sse2(n1, n2, n3, haystack, len);
#elif defined(USE_NEON)
    return memchr3_neon(n1, n2, n3, haystack, len);
#else
    return memchr3_scalar(n1, n2, n3, haystack, len);
#endif
}

/*
 * Public memrchr2 function - dispatches to best implementation
 */
const uint8_t* memrchr2_find(uint8_t n1, uint8_t n2, const uint8_t* haystack, size_t len) {
#ifdef USE_SSE2
    return memrchr2_sse2(n1, n2, haystack, len);
#elif defined(USE_NEON)
    return memrchr2_neon(n1, n2, haystack, len);
#else
    return memrchr2_scalar(n1, n2, haystack, len);
#endif
}

/*
 * Public memrchr3 function - dispatches to best implementation
 */
const uint8_t* memrchr3_find(uint8_t n1, uint8_t n2, uint8_t n3,
                             const uint8_t* haystack, size_t len) {
#ifdef USE_SSE2
    return memrchr3_sse2(n1, n2, n3, haystack, len);
#elif defined(USE_NEON)
    return memrchr3_neon(n1, n2, n3, haystack, len);
#else
    return memrchr3_scalar(n1, n2, n3, haystack, len);
#endif
}

/*
 * Substring search - forward
 * Uses memmem if available, otherwise Two-Way algorithm
 */
const uint8_t* memmem_find(const uint8_t* needle, size_t needle_len,
                           const uint8_t* haystack, size_t haystack_len) {
    /* Empty needle matches at position 0 */
    if (needle_len == 0) {
        return haystack;
    }

    /* Needle longer than haystack - no match */
    if (needle_len > haystack_len) {
        return NULL;
    }

    /* Single byte needle - use memchr */
    if (needle_len == 1) {
        return memchr_find(needle[0], haystack, haystack_len);
    }

#if defined(_GNU_SOURCE) || defined(__GLIBC__)
    /* Use glibc memmem which is highly optimized */
    return (const uint8_t*)memmem(haystack, haystack_len, needle, needle_len);
#else
    /* Simple Boyer-Moore-Horspool implementation */
    size_t skip[256];

    /* Initialize skip table */
    for (int i = 0; i < 256; i++) {
        skip[i] = needle_len;
    }
    for (size_t i = 0; i < needle_len - 1; i++) {
        skip[needle[i]] = needle_len - 1 - i;
    }

    /* Search */
    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + haystack_len - needle_len;

    while (ptr <= end) {
        size_t j = needle_len - 1;
        while (ptr[j] == needle[j]) {
            if (j == 0) return ptr;
            j--;
        }
        ptr += skip[ptr[needle_len - 1]];
    }

    return NULL;
#endif
}

/*
 * Substring search - reverse
 */
const uint8_t* memmem_rfind(const uint8_t* needle, size_t needle_len,
                            const uint8_t* haystack, size_t haystack_len) {
    /* Empty needle matches at end */
    if (needle_len == 0) {
        return haystack + haystack_len;
    }

    /* Needle longer than haystack - no match */
    if (needle_len > haystack_len) {
        return NULL;
    }

    /* Single byte needle - use memrchr */
    if (needle_len == 1) {
        return memrchr_find(needle[0], haystack, haystack_len);
    }

    /* Search from end */
    const uint8_t* ptr = haystack + haystack_len - needle_len;

    while (ptr >= haystack) {
        if (memcmp(ptr, needle, needle_len) == 0) {
            return ptr;
        }
        ptr--;
    }

    return NULL;
}
