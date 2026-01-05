/*
 * memchr.c - High-performance byte and substring search functions
 *
 * Implementation uses SIMD optimizations where available:
 * - SSE2/AVX2 on x86-64 with runtime CPU feature detection
 * - NEON on ARM64
 * - Fallback scalar implementation for other platforms
 *
 * Substring search uses Two-Way algorithm with SIMD prefiltering
 * for optimal performance on both short and long needles.
 */

#include "memchr.h"
#include <string.h>
#include <stdbool.h>

/* Detect platform and SIMD capabilities */
#if defined(__x86_64__) || defined(_M_X64)
    #define USE_X86_64 1
    #include <emmintrin.h>  /* SSE2 */
    #include <immintrin.h>  /* AVX2 */
    #if defined(_MSC_VER)
        #include <intrin.h>
    #else
        #include <cpuid.h>
    #endif
#elif defined(__aarch64__) || defined(_M_ARM64)
    #define USE_NEON 1
    #include <arm_neon.h>
#endif

/*
 * Runtime CPU feature detection for x86-64
 */
#ifdef USE_X86_64

static int cpu_features_detected = 0;
static int has_avx2 = 0;

static void detect_cpu_features(void) {
    if (cpu_features_detected) return;

    unsigned int eax, ebx, ecx, edx;

    /* Check for AVX2 support */
    #if defined(_MSC_VER)
        int cpuInfo[4];
        __cpuid(cpuInfo, 0);
        int nIds = cpuInfo[0];

        if (nIds >= 7) {
            __cpuidex(cpuInfo, 7, 0);
            has_avx2 = (cpuInfo[1] & (1 << 5)) != 0;  /* AVX2 bit in EBX */
        }
    #else
        if (__get_cpuid_max(0, NULL) >= 7) {
            __cpuid_count(7, 0, eax, ebx, ecx, edx);
            has_avx2 = (ebx & (1 << 5)) != 0;  /* AVX2 bit in EBX */
        }
    #endif

    cpu_features_detected = 1;
}

#define ENSURE_CPU_DETECTED() do { if (!cpu_features_detected) detect_cpu_features(); } while(0)

#endif /* USE_X86_64 */

/*
 * Single byte search - forward
 * Uses libc memchr which is highly optimized on most platforms
 */
const uint8_t* memchr_find(uint8_t needle, const uint8_t* haystack, size_t len) {
    return (const uint8_t*)memchr(haystack, needle, len);
}

#ifdef USE_X86_64

/*
 * AVX2 implementation for memrchr - reverse single byte search
 * Processes 64 bytes per iteration (2x unrolled) for better throughput
 */
static inline const uint8_t* memrchr_avx2(uint8_t needle, const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack + len;
    __m256i needle_vec = _mm256_set1_epi8(needle);

    /* Handle unaligned suffix first */
    while (ptr > haystack && ((uintptr_t)ptr & 31) != 0) {
        ptr--;
        if (*ptr == needle) return ptr;
    }

    /* Main loop: process 64 bytes per iteration (unrolled 2x) */
    while (ptr >= haystack + 64) {
        ptr -= 64;

        /* Load and compare second chunk first (we want rightmost match) */
        __m256i chunk1 = _mm256_loadu_si256((const __m256i*)(ptr + 32));
        __m256i eq1 = _mm256_cmpeq_epi8(chunk1, needle_vec);
        int mask1 = _mm256_movemask_epi8(eq1);

        if (mask1 != 0) {
            int idx = 31 - __builtin_clz(mask1);
            return ptr + 32 + idx;
        }

        /* Then check first chunk */
        __m256i chunk0 = _mm256_loadu_si256((const __m256i*)ptr);
        __m256i eq0 = _mm256_cmpeq_epi8(chunk0, needle_vec);
        int mask0 = _mm256_movemask_epi8(eq0);

        if (mask0 != 0) {
            int idx = 31 - __builtin_clz(mask0);
            return ptr + idx;
        }
    }

    /* Handle remaining 32+ bytes */
    while (ptr >= haystack + 32) {
        ptr -= 32;
        __m256i chunk = _mm256_loadu_si256((const __m256i*)ptr);
        __m256i eq = _mm256_cmpeq_epi8(chunk, needle_vec);
        int mask = _mm256_movemask_epi8(eq);

        if (mask != 0) {
            int idx = 31 - __builtin_clz(mask);
            return ptr + idx;
        }
    }

    /* Scalar tail */
    while (ptr > haystack) {
        ptr--;
        if (*ptr == needle) return ptr;
    }

    return NULL;
}

/*
 * SSE2 implementation for memrchr - reverse single byte search
 */
static inline const uint8_t* memrchr_sse2(uint8_t needle, const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack + len;
    __m128i needle_vec = _mm_set1_epi8(needle);

    /* Handle unaligned suffix first */
    while (ptr > haystack && ((uintptr_t)ptr & 15) != 0) {
        ptr--;
        if (*ptr == needle) return ptr;
    }

    /* Main loop: process 32 bytes per iteration (unrolled 2x) */
    while (ptr >= haystack + 32) {
        ptr -= 32;

        /* Check second chunk first (rightmost) */
        __m128i chunk1 = _mm_loadu_si128((const __m128i*)(ptr + 16));
        __m128i eq1 = _mm_cmpeq_epi8(chunk1, needle_vec);
        int mask1 = _mm_movemask_epi8(eq1);

        if (mask1 != 0) {
            int idx = 31 - __builtin_clz(mask1);
            return ptr + 16 + idx;
        }

        /* Then check first chunk */
        __m128i chunk0 = _mm_loadu_si128((const __m128i*)ptr);
        __m128i eq0 = _mm_cmpeq_epi8(chunk0, needle_vec);
        int mask0 = _mm_movemask_epi8(eq0);

        if (mask0 != 0) {
            int idx = 31 - __builtin_clz(mask0);
            return ptr + idx;
        }
    }

    /* Handle remaining 16 bytes */
    while (ptr >= haystack + 16) {
        ptr -= 16;
        __m128i chunk = _mm_loadu_si128((const __m128i*)ptr);
        __m128i eq = _mm_cmpeq_epi8(chunk, needle_vec);
        int mask = _mm_movemask_epi8(eq);

        if (mask != 0) {
            int idx = 31 - __builtin_clz(mask);
            return ptr + idx;
        }
    }

    /* Scalar tail */
    while (ptr > haystack) {
        ptr--;
        if (*ptr == needle) return ptr;
    }

    return NULL;
}

#endif /* USE_X86_64 */

/*
 * Single byte search - reverse
 * Uses custom SIMD implementation for better performance
 */
const uint8_t* memrchr_find(uint8_t needle, const uint8_t* haystack, size_t len) {
#ifdef USE_X86_64
    ENSURE_CPU_DETECTED();
    if (has_avx2) {
        return memrchr_avx2(needle, haystack, len);
    }
    return memrchr_sse2(needle, haystack, len);
#elif defined(_GNU_SOURCE) || defined(__GLIBC__)
    return (const uint8_t*)memrchr(haystack, needle, len);
#else
    /* Fallback scalar implementation */
    if (len == 0) return NULL;
    const uint8_t* ptr = haystack + len;
    while (ptr > haystack) {
        ptr--;
        if (*ptr == needle) return ptr;
    }
    return NULL;
#endif
}

#ifdef USE_X86_64

/*
 * AVX2 implementation for memchr2 (32 bytes at a time)
 */
static inline const uint8_t* memchr2_avx2(uint8_t n1, uint8_t n2,
                                           const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + len;

    /* Handle unaligned prefix */
    while (ptr < end && ((uintptr_t)ptr & 31) != 0) {
        if (*ptr == n1 || *ptr == n2) return ptr;
        ptr++;
    }

    if (ptr >= end) return NULL;

    /* AVX2 vectorized search - 32 bytes at a time */
    __m256i needle1 = _mm256_set1_epi8(n1);
    __m256i needle2 = _mm256_set1_epi8(n2);

    while (ptr + 32 <= end) {
        __m256i chunk = _mm256_load_si256((const __m256i*)ptr);
        __m256i cmp1 = _mm256_cmpeq_epi8(chunk, needle1);
        __m256i cmp2 = _mm256_cmpeq_epi8(chunk, needle2);
        __m256i result = _mm256_or_si256(cmp1, cmp2);
        int mask = _mm256_movemask_epi8(result);

        if (mask != 0) {
            int idx = __builtin_ctz(mask);
            return ptr + idx;
        }
        ptr += 32;
    }

    /* Handle remaining bytes with SSE2 */
    __m128i needle1_sse = _mm_set1_epi8(n1);
    __m128i needle2_sse = _mm_set1_epi8(n2);

    while (ptr + 16 <= end) {
        __m128i chunk = _mm_loadu_si128((const __m128i*)ptr);
        __m128i cmp1 = _mm_cmpeq_epi8(chunk, needle1_sse);
        __m128i cmp2 = _mm_cmpeq_epi8(chunk, needle2_sse);
        __m128i result_sse = _mm_or_si128(cmp1, cmp2);
        int mask = _mm_movemask_epi8(result_sse);

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
 * SSE2 implementation for memchr2
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
 * AVX2 implementation for memchr3
 * Processes 64 bytes per iteration (2x unrolled) for better throughput
 */
static inline const uint8_t* memchr3_avx2(uint8_t n1, uint8_t n2, uint8_t n3,
                                           const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + len;

    /* Handle unaligned prefix */
    while (ptr < end && ((uintptr_t)ptr & 31) != 0) {
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
        ptr++;
    }

    if (ptr >= end) return NULL;

    /* AVX2 vectorized search */
    __m256i needle1 = _mm256_set1_epi8(n1);
    __m256i needle2 = _mm256_set1_epi8(n2);
    __m256i needle3 = _mm256_set1_epi8(n3);

    /* Prefetch hint */
    _mm_prefetch((const char*)(ptr + 256), _MM_HINT_T0);

    /* Main loop: process 64 bytes per iteration (unrolled 2x) */
    while (ptr + 64 <= end) {
        _mm_prefetch((const char*)(ptr + 320), _MM_HINT_T0);

        /* First 32 bytes */
        __m256i chunk0 = _mm256_load_si256((const __m256i*)ptr);
        __m256i cmp1_0 = _mm256_cmpeq_epi8(chunk0, needle1);
        __m256i cmp2_0 = _mm256_cmpeq_epi8(chunk0, needle2);
        __m256i cmp3_0 = _mm256_cmpeq_epi8(chunk0, needle3);
        __m256i result0 = _mm256_or_si256(_mm256_or_si256(cmp1_0, cmp2_0), cmp3_0);
        int mask0 = _mm256_movemask_epi8(result0);

        if (mask0 != 0) {
            int idx = __builtin_ctz(mask0);
            return ptr + idx;
        }

        /* Second 32 bytes */
        __m256i chunk1 = _mm256_load_si256((const __m256i*)(ptr + 32));
        __m256i cmp1_1 = _mm256_cmpeq_epi8(chunk1, needle1);
        __m256i cmp2_1 = _mm256_cmpeq_epi8(chunk1, needle2);
        __m256i cmp3_1 = _mm256_cmpeq_epi8(chunk1, needle3);
        __m256i result1 = _mm256_or_si256(_mm256_or_si256(cmp1_1, cmp2_1), cmp3_1);
        int mask1 = _mm256_movemask_epi8(result1);

        if (mask1 != 0) {
            int idx = __builtin_ctz(mask1);
            return ptr + 32 + idx;
        }

        ptr += 64;
    }

    /* Handle remaining 32 bytes */
    while (ptr + 32 <= end) {
        __m256i chunk = _mm256_loadu_si256((const __m256i*)ptr);
        __m256i cmp1 = _mm256_cmpeq_epi8(chunk, needle1);
        __m256i cmp2 = _mm256_cmpeq_epi8(chunk, needle2);
        __m256i cmp3 = _mm256_cmpeq_epi8(chunk, needle3);
        __m256i result = _mm256_or_si256(_mm256_or_si256(cmp1, cmp2), cmp3);
        int mask = _mm256_movemask_epi8(result);

        if (mask != 0) {
            int idx = __builtin_ctz(mask);
            return ptr + idx;
        }
        ptr += 32;
    }

    /* Handle remaining with scalar */
    while (ptr < end) {
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
        ptr++;
    }

    return NULL;
}

/*
 * SSE2 implementation for memchr3
 * Processes 32 bytes per iteration (2x unrolled) for better throughput
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

    /* Main loop: process 32 bytes per iteration (unrolled 2x) */
    while (ptr + 32 <= end) {
        /* First 16 bytes */
        __m128i chunk0 = _mm_load_si128((const __m128i*)ptr);
        __m128i cmp1_0 = _mm_cmpeq_epi8(chunk0, needle1);
        __m128i cmp2_0 = _mm_cmpeq_epi8(chunk0, needle2);
        __m128i cmp3_0 = _mm_cmpeq_epi8(chunk0, needle3);
        __m128i result0 = _mm_or_si128(_mm_or_si128(cmp1_0, cmp2_0), cmp3_0);
        int mask0 = _mm_movemask_epi8(result0);

        if (mask0 != 0) {
            int idx = __builtin_ctz(mask0);
            return ptr + idx;
        }

        /* Second 16 bytes */
        __m128i chunk1 = _mm_load_si128((const __m128i*)(ptr + 16));
        __m128i cmp1_1 = _mm_cmpeq_epi8(chunk1, needle1);
        __m128i cmp2_1 = _mm_cmpeq_epi8(chunk1, needle2);
        __m128i cmp3_1 = _mm_cmpeq_epi8(chunk1, needle3);
        __m128i result1 = _mm_or_si128(_mm_or_si128(cmp1_1, cmp2_1), cmp3_1);
        int mask1 = _mm_movemask_epi8(result1);

        if (mask1 != 0) {
            int idx = __builtin_ctz(mask1);
            return ptr + 16 + idx;
        }

        ptr += 32;
    }

    /* Handle remaining 16 bytes */
    while (ptr + 16 <= end) {
        __m128i chunk = _mm_loadu_si128((const __m128i*)ptr);
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
 * AVX2 implementation for memrchr2
 * Processes 64 bytes per iteration (2x unrolled)
 */
static inline const uint8_t* memrchr2_avx2(uint8_t n1, uint8_t n2,
                                            const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack + len;
    __m256i needle1 = _mm256_set1_epi8(n1);
    __m256i needle2 = _mm256_set1_epi8(n2);

    /* Handle unaligned suffix first */
    while (ptr > haystack && ((uintptr_t)ptr & 31) != 0) {
        ptr--;
        if (*ptr == n1 || *ptr == n2) return ptr;
    }

    /* Main loop: process 64 bytes per iteration (unrolled 2x) */
    while (ptr >= haystack + 64) {
        ptr -= 64;

        /* Check second chunk first (rightmost) */
        __m256i chunk1 = _mm256_loadu_si256((const __m256i*)(ptr + 32));
        __m256i cmp1_1 = _mm256_cmpeq_epi8(chunk1, needle1);
        __m256i cmp2_1 = _mm256_cmpeq_epi8(chunk1, needle2);
        __m256i result1 = _mm256_or_si256(cmp1_1, cmp2_1);
        int mask1 = _mm256_movemask_epi8(result1);

        if (mask1 != 0) {
            int idx = 31 - __builtin_clz(mask1);
            return ptr + 32 + idx;
        }

        /* Then check first chunk */
        __m256i chunk0 = _mm256_loadu_si256((const __m256i*)ptr);
        __m256i cmp1_0 = _mm256_cmpeq_epi8(chunk0, needle1);
        __m256i cmp2_0 = _mm256_cmpeq_epi8(chunk0, needle2);
        __m256i result0 = _mm256_or_si256(cmp1_0, cmp2_0);
        int mask0 = _mm256_movemask_epi8(result0);

        if (mask0 != 0) {
            int idx = 31 - __builtin_clz(mask0);
            return ptr + idx;
        }
    }

    /* Handle remaining 32+ bytes */
    while (ptr >= haystack + 32) {
        ptr -= 32;
        __m256i chunk = _mm256_loadu_si256((const __m256i*)ptr);
        __m256i cmp1 = _mm256_cmpeq_epi8(chunk, needle1);
        __m256i cmp2 = _mm256_cmpeq_epi8(chunk, needle2);
        __m256i result = _mm256_or_si256(cmp1, cmp2);
        int mask = _mm256_movemask_epi8(result);

        if (mask != 0) {
            int idx = 31 - __builtin_clz(mask);
            return ptr + idx;
        }
    }

    /* Scalar tail */
    while (ptr > haystack) {
        ptr--;
        if (*ptr == n1 || *ptr == n2) return ptr;
    }

    return NULL;
}

/*
 * SSE2 implementation for memrchr2
 */
static inline const uint8_t* memrchr2_sse2(uint8_t n1, uint8_t n2,
                                            const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack + len;
    __m128i needle1 = _mm_set1_epi8(n1);
    __m128i needle2 = _mm_set1_epi8(n2);

    /* Handle unaligned suffix */
    while (ptr > haystack && ((uintptr_t)ptr & 15) != 0) {
        ptr--;
        if (*ptr == n1 || *ptr == n2) return ptr;
    }

    /* Main loop: process 32 bytes per iteration (unrolled 2x) */
    while (ptr >= haystack + 32) {
        ptr -= 32;

        /* Check second chunk first (rightmost) */
        __m128i chunk1 = _mm_loadu_si128((const __m128i*)(ptr + 16));
        __m128i cmp1_1 = _mm_cmpeq_epi8(chunk1, needle1);
        __m128i cmp2_1 = _mm_cmpeq_epi8(chunk1, needle2);
        __m128i result1 = _mm_or_si128(cmp1_1, cmp2_1);
        int mask1 = _mm_movemask_epi8(result1);

        if (mask1 != 0) {
            int idx = 31 - __builtin_clz(mask1);
            return ptr + 16 + idx;
        }

        /* Then check first chunk */
        __m128i chunk0 = _mm_loadu_si128((const __m128i*)ptr);
        __m128i cmp1_0 = _mm_cmpeq_epi8(chunk0, needle1);
        __m128i cmp2_0 = _mm_cmpeq_epi8(chunk0, needle2);
        __m128i result0 = _mm_or_si128(cmp1_0, cmp2_0);
        int mask0 = _mm_movemask_epi8(result0);

        if (mask0 != 0) {
            int idx = 31 - __builtin_clz(mask0);
            return ptr + idx;
        }
    }

    /* Handle remaining 16 bytes */
    while (ptr >= haystack + 16) {
        ptr -= 16;
        __m128i chunk = _mm_loadu_si128((const __m128i*)ptr);
        __m128i cmp1 = _mm_cmpeq_epi8(chunk, needle1);
        __m128i cmp2 = _mm_cmpeq_epi8(chunk, needle2);
        __m128i result = _mm_or_si128(cmp1, cmp2);
        int mask = _mm_movemask_epi8(result);

        if (mask != 0) {
            int idx = 31 - __builtin_clz(mask);
            return ptr + idx;
        }
    }

    /* Scalar tail */
    while (ptr > haystack) {
        ptr--;
        if (*ptr == n1 || *ptr == n2) return ptr;
    }

    return NULL;
}

/*
 * AVX2 implementation for memrchr3
 * Processes 64 bytes per iteration (2x unrolled)
 */
static inline const uint8_t* memrchr3_avx2(uint8_t n1, uint8_t n2, uint8_t n3,
                                            const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack + len;
    __m256i needle1 = _mm256_set1_epi8(n1);
    __m256i needle2 = _mm256_set1_epi8(n2);
    __m256i needle3 = _mm256_set1_epi8(n3);

    /* Handle unaligned suffix first */
    while (ptr > haystack && ((uintptr_t)ptr & 31) != 0) {
        ptr--;
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
    }

    /* Main loop: process 64 bytes per iteration (unrolled 2x) */
    while (ptr >= haystack + 64) {
        ptr -= 64;

        /* Check second chunk first (rightmost) */
        __m256i chunk1 = _mm256_loadu_si256((const __m256i*)(ptr + 32));
        __m256i cmp1_1 = _mm256_cmpeq_epi8(chunk1, needle1);
        __m256i cmp2_1 = _mm256_cmpeq_epi8(chunk1, needle2);
        __m256i cmp3_1 = _mm256_cmpeq_epi8(chunk1, needle3);
        __m256i result1 = _mm256_or_si256(_mm256_or_si256(cmp1_1, cmp2_1), cmp3_1);
        int mask1 = _mm256_movemask_epi8(result1);

        if (mask1 != 0) {
            int idx = 31 - __builtin_clz(mask1);
            return ptr + 32 + idx;
        }

        /* Then check first chunk */
        __m256i chunk0 = _mm256_loadu_si256((const __m256i*)ptr);
        __m256i cmp1_0 = _mm256_cmpeq_epi8(chunk0, needle1);
        __m256i cmp2_0 = _mm256_cmpeq_epi8(chunk0, needle2);
        __m256i cmp3_0 = _mm256_cmpeq_epi8(chunk0, needle3);
        __m256i result0 = _mm256_or_si256(_mm256_or_si256(cmp1_0, cmp2_0), cmp3_0);
        int mask0 = _mm256_movemask_epi8(result0);

        if (mask0 != 0) {
            int idx = 31 - __builtin_clz(mask0);
            return ptr + idx;
        }
    }

    /* Handle remaining 32+ bytes */
    while (ptr >= haystack + 32) {
        ptr -= 32;
        __m256i chunk = _mm256_loadu_si256((const __m256i*)ptr);
        __m256i cmp1 = _mm256_cmpeq_epi8(chunk, needle1);
        __m256i cmp2 = _mm256_cmpeq_epi8(chunk, needle2);
        __m256i cmp3 = _mm256_cmpeq_epi8(chunk, needle3);
        __m256i result = _mm256_or_si256(_mm256_or_si256(cmp1, cmp2), cmp3);
        int mask = _mm256_movemask_epi8(result);

        if (mask != 0) {
            int idx = 31 - __builtin_clz(mask);
            return ptr + idx;
        }
    }

    /* Scalar tail */
    while (ptr > haystack) {
        ptr--;
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
    }

    return NULL;
}

/*
 * SSE2 implementation for memrchr3
 */
static inline const uint8_t* memrchr3_sse2(uint8_t n1, uint8_t n2, uint8_t n3,
                                            const uint8_t* haystack, size_t len) {
    if (len == 0) return NULL;

    const uint8_t* ptr = haystack + len;
    __m128i needle1 = _mm_set1_epi8(n1);
    __m128i needle2 = _mm_set1_epi8(n2);
    __m128i needle3 = _mm_set1_epi8(n3);

    /* Handle unaligned suffix */
    while (ptr > haystack && ((uintptr_t)ptr & 15) != 0) {
        ptr--;
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
    }

    /* Main loop: process 32 bytes per iteration (unrolled 2x) */
    while (ptr >= haystack + 32) {
        ptr -= 32;

        /* Check second chunk first (rightmost) */
        __m128i chunk1 = _mm_loadu_si128((const __m128i*)(ptr + 16));
        __m128i cmp1_1 = _mm_cmpeq_epi8(chunk1, needle1);
        __m128i cmp2_1 = _mm_cmpeq_epi8(chunk1, needle2);
        __m128i cmp3_1 = _mm_cmpeq_epi8(chunk1, needle3);
        __m128i result1 = _mm_or_si128(_mm_or_si128(cmp1_1, cmp2_1), cmp3_1);
        int mask1 = _mm_movemask_epi8(result1);

        if (mask1 != 0) {
            int idx = 31 - __builtin_clz(mask1);
            return ptr + 16 + idx;
        }

        /* Then check first chunk */
        __m128i chunk0 = _mm_loadu_si128((const __m128i*)ptr);
        __m128i cmp1_0 = _mm_cmpeq_epi8(chunk0, needle1);
        __m128i cmp2_0 = _mm_cmpeq_epi8(chunk0, needle2);
        __m128i cmp3_0 = _mm_cmpeq_epi8(chunk0, needle3);
        __m128i result0 = _mm_or_si128(_mm_or_si128(cmp1_0, cmp2_0), cmp3_0);
        int mask0 = _mm_movemask_epi8(result0);

        if (mask0 != 0) {
            int idx = 31 - __builtin_clz(mask0);
            return ptr + idx;
        }
    }

    /* Handle remaining 16 bytes */
    while (ptr >= haystack + 16) {
        ptr -= 16;
        __m128i chunk = _mm_loadu_si128((const __m128i*)ptr);
        __m128i cmp1 = _mm_cmpeq_epi8(chunk, needle1);
        __m128i cmp2 = _mm_cmpeq_epi8(chunk, needle2);
        __m128i cmp3 = _mm_cmpeq_epi8(chunk, needle3);
        __m128i result = _mm_or_si128(_mm_or_si128(cmp1, cmp2), cmp3);
        int mask = _mm_movemask_epi8(result);

        if (mask != 0) {
            int idx = 31 - __builtin_clz(mask);
            return ptr + idx;
        }
    }

    /* Scalar tail */
    while (ptr > haystack) {
        ptr--;
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
    }

    return NULL;
}

/*
 * Packed Pair SIMD Algorithm for substring search
 *
 * This is the key algorithm from Rust's memchr library. Instead of searching
 * for a single byte, we search for a PAIR of bytes: the first and last byte
 * of the needle. This provides much better filtering because:
 *
 * 1. Two bytes matching at a specific distance is much rarer than one byte
 * 2. No frequency table overhead - just use first/last bytes
 * 3. The fixed offset (needle_len - 1) implicitly validates spacing
 *
 * For a needle "hello":
 * - first_byte = 'h', last_byte = 'o', offset = 4
 * - We search for positions where haystack[i] == 'h' AND haystack[i+4] == 'o'
 * - This is much more selective than searching for just 'h'
 */

/*
 * AVX2 Packed Pair implementation - processes 32 bytes at a time
 * Uses loop unrolling to process 64 bytes per iteration for better throughput
 */
static inline const uint8_t* memmem_avx2(const uint8_t* needle, size_t needle_len,
                                          const uint8_t* haystack, size_t haystack_len) {
    const uint8_t first_byte = needle[0];
    const uint8_t last_byte = needle[needle_len - 1];
    const size_t offset = needle_len - 1;

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + haystack_len - needle_len + 1;

    /* Broadcast first and last bytes to AVX2 vectors */
    __m256i first_vec = _mm256_set1_epi8(first_byte);
    __m256i last_vec = _mm256_set1_epi8(last_byte);

    /* Prefetch hint for upcoming data */
    _mm_prefetch((const char*)(ptr + 256), _MM_HINT_T0);

    /* Main loop: process 64 bytes per iteration (unrolled 2x) */
    while (ptr + 64 <= end) {
        _mm_prefetch((const char*)(ptr + 320), _MM_HINT_T0);

        /* First 32 bytes */
        __m256i chunk_first0 = _mm256_loadu_si256((const __m256i*)ptr);
        __m256i chunk_last0 = _mm256_loadu_si256((const __m256i*)(ptr + offset));
        __m256i eq_first0 = _mm256_cmpeq_epi8(chunk_first0, first_vec);
        __m256i eq_last0 = _mm256_cmpeq_epi8(chunk_last0, last_vec);
        __m256i candidates0 = _mm256_and_si256(eq_first0, eq_last0);
        int mask0 = _mm256_movemask_epi8(candidates0);

        /* Second 32 bytes */
        __m256i chunk_first1 = _mm256_loadu_si256((const __m256i*)(ptr + 32));
        __m256i chunk_last1 = _mm256_loadu_si256((const __m256i*)(ptr + 32 + offset));
        __m256i eq_first1 = _mm256_cmpeq_epi8(chunk_first1, first_vec);
        __m256i eq_last1 = _mm256_cmpeq_epi8(chunk_last1, last_vec);
        __m256i candidates1 = _mm256_and_si256(eq_first1, eq_last1);
        int mask1 = _mm256_movemask_epi8(candidates1);

        /* Process candidates from first chunk */
        while (mask0 != 0) {
            int idx = __builtin_ctz(mask0);
            const uint8_t* candidate = ptr + idx;
            /* Already verified first and last bytes match, check middle */
            if (needle_len <= 2 || memcmp(candidate + 1, needle + 1, needle_len - 2) == 0) {
                return candidate;
            }
            mask0 &= mask0 - 1;
        }

        /* Process candidates from second chunk */
        while (mask1 != 0) {
            int idx = __builtin_ctz(mask1);
            const uint8_t* candidate = ptr + 32 + idx;
            if (needle_len <= 2 || memcmp(candidate + 1, needle + 1, needle_len - 2) == 0) {
                return candidate;
            }
            mask1 &= mask1 - 1;
        }

        ptr += 64;
    }

    /* Handle remaining 32+ bytes */
    while (ptr + 32 <= end) {
        __m256i chunk_first = _mm256_loadu_si256((const __m256i*)ptr);
        __m256i chunk_last = _mm256_loadu_si256((const __m256i*)(ptr + offset));
        __m256i eq_first = _mm256_cmpeq_epi8(chunk_first, first_vec);
        __m256i eq_last = _mm256_cmpeq_epi8(chunk_last, last_vec);
        __m256i candidates = _mm256_and_si256(eq_first, eq_last);
        int mask = _mm256_movemask_epi8(candidates);

        while (mask != 0) {
            int idx = __builtin_ctz(mask);
            const uint8_t* candidate = ptr + idx;
            if (needle_len <= 2 || memcmp(candidate + 1, needle + 1, needle_len - 2) == 0) {
                return candidate;
            }
            mask &= mask - 1;
        }
        ptr += 32;
    }

    /* Handle remaining bytes with SSE2 */
    __m128i first_vec_sse = _mm_set1_epi8(first_byte);
    __m128i last_vec_sse = _mm_set1_epi8(last_byte);

    while (ptr + 16 <= end) {
        __m128i chunk_first = _mm_loadu_si128((const __m128i*)ptr);
        __m128i chunk_last = _mm_loadu_si128((const __m128i*)(ptr + offset));
        __m128i eq_first = _mm_cmpeq_epi8(chunk_first, first_vec_sse);
        __m128i eq_last = _mm_cmpeq_epi8(chunk_last, last_vec_sse);
        __m128i candidates = _mm_and_si128(eq_first, eq_last);
        int mask = _mm_movemask_epi8(candidates);

        while (mask != 0) {
            int idx = __builtin_ctz(mask);
            const uint8_t* candidate = ptr + idx;
            if (needle_len <= 2 || memcmp(candidate + 1, needle + 1, needle_len - 2) == 0) {
                return candidate;
            }
            mask &= mask - 1;
        }
        ptr += 16;
    }

    /* Scalar tail */
    while (ptr < end) {
        if (*ptr == first_byte && ptr[offset] == last_byte) {
            if (needle_len <= 2 || memcmp(ptr + 1, needle + 1, needle_len - 2) == 0) {
                return ptr;
            }
        }
        ptr++;
    }

    return NULL;
}

/*
 * SSE2 Packed Pair implementation - processes 16 bytes at a time
 * With loop unrolling to process 32 bytes per iteration
 */
static inline const uint8_t* memmem_sse2(const uint8_t* needle, size_t needle_len,
                                          const uint8_t* haystack, size_t haystack_len) {
    const uint8_t first_byte = needle[0];
    const uint8_t last_byte = needle[needle_len - 1];
    const size_t offset = needle_len - 1;

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + haystack_len - needle_len + 1;

    /* Broadcast first and last bytes to SSE2 vectors */
    __m128i first_vec = _mm_set1_epi8(first_byte);
    __m128i last_vec = _mm_set1_epi8(last_byte);

    /* Main loop: process 32 bytes per iteration (unrolled 2x) */
    while (ptr + 32 <= end) {
        /* First 16 bytes */
        __m128i chunk_first0 = _mm_loadu_si128((const __m128i*)ptr);
        __m128i chunk_last0 = _mm_loadu_si128((const __m128i*)(ptr + offset));
        __m128i eq_first0 = _mm_cmpeq_epi8(chunk_first0, first_vec);
        __m128i eq_last0 = _mm_cmpeq_epi8(chunk_last0, last_vec);
        __m128i candidates0 = _mm_and_si128(eq_first0, eq_last0);
        int mask0 = _mm_movemask_epi8(candidates0);

        /* Second 16 bytes */
        __m128i chunk_first1 = _mm_loadu_si128((const __m128i*)(ptr + 16));
        __m128i chunk_last1 = _mm_loadu_si128((const __m128i*)(ptr + 16 + offset));
        __m128i eq_first1 = _mm_cmpeq_epi8(chunk_first1, first_vec);
        __m128i eq_last1 = _mm_cmpeq_epi8(chunk_last1, last_vec);
        __m128i candidates1 = _mm_and_si128(eq_first1, eq_last1);
        int mask1 = _mm_movemask_epi8(candidates1);

        /* Process candidates from first chunk */
        while (mask0 != 0) {
            int idx = __builtin_ctz(mask0);
            const uint8_t* candidate = ptr + idx;
            if (needle_len <= 2 || memcmp(candidate + 1, needle + 1, needle_len - 2) == 0) {
                return candidate;
            }
            mask0 &= mask0 - 1;
        }

        /* Process candidates from second chunk */
        while (mask1 != 0) {
            int idx = __builtin_ctz(mask1);
            const uint8_t* candidate = ptr + 16 + idx;
            if (needle_len <= 2 || memcmp(candidate + 1, needle + 1, needle_len - 2) == 0) {
                return candidate;
            }
            mask1 &= mask1 - 1;
        }

        ptr += 32;
    }

    /* Handle remaining 16 bytes */
    while (ptr + 16 <= end) {
        __m128i chunk_first = _mm_loadu_si128((const __m128i*)ptr);
        __m128i chunk_last = _mm_loadu_si128((const __m128i*)(ptr + offset));
        __m128i eq_first = _mm_cmpeq_epi8(chunk_first, first_vec);
        __m128i eq_last = _mm_cmpeq_epi8(chunk_last, last_vec);
        __m128i candidates = _mm_and_si128(eq_first, eq_last);
        int mask = _mm_movemask_epi8(candidates);

        while (mask != 0) {
            int idx = __builtin_ctz(mask);
            const uint8_t* candidate = ptr + idx;
            if (needle_len <= 2 || memcmp(candidate + 1, needle + 1, needle_len - 2) == 0) {
                return candidate;
            }
            mask &= mask - 1;
        }
        ptr += 16;
    }

    /* Scalar tail */
    while (ptr < end) {
        if (*ptr == first_byte && ptr[offset] == last_byte) {
            if (needle_len <= 2 || memcmp(ptr + 1, needle + 1, needle_len - 2) == 0) {
                return ptr;
            }
        }
        ptr++;
    }

    return NULL;
}

#endif /* USE_X86_64 */

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

/*
 * NEON Packed Pair implementation - processes 16 bytes at a time
 */
static inline const uint8_t* memmem_neon(const uint8_t* needle, size_t needle_len,
                                          const uint8_t* haystack, size_t haystack_len) {
    const uint8_t first_byte = needle[0];
    const uint8_t last_byte = needle[needle_len - 1];
    const size_t offset = needle_len - 1;

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + haystack_len - needle_len + 1;

    uint8x16_t first_vec = vdupq_n_u8(first_byte);
    uint8x16_t last_vec = vdupq_n_u8(last_byte);

    /* Main loop: process 32 bytes per iteration (unrolled 2x) */
    while (ptr + 32 <= end) {
        /* First 16 bytes */
        uint8x16_t chunk_first0 = vld1q_u8(ptr);
        uint8x16_t chunk_last0 = vld1q_u8(ptr + offset);
        uint8x16_t eq_first0 = vceqq_u8(chunk_first0, first_vec);
        uint8x16_t eq_last0 = vceqq_u8(chunk_last0, last_vec);
        uint8x16_t candidates0 = vandq_u8(eq_first0, eq_last0);

        /* Second 16 bytes */
        uint8x16_t chunk_first1 = vld1q_u8(ptr + 16);
        uint8x16_t chunk_last1 = vld1q_u8(ptr + 16 + offset);
        uint8x16_t eq_first1 = vceqq_u8(chunk_first1, first_vec);
        uint8x16_t eq_last1 = vceqq_u8(chunk_last1, last_vec);
        uint8x16_t candidates1 = vandq_u8(eq_first1, eq_last1);

        /* Check if any candidates in first chunk */
        uint64x2_t cand64_0 = vreinterpretq_u64_u8(candidates0);
        if (vgetq_lane_u64(cand64_0, 0) || vgetq_lane_u64(cand64_0, 1)) {
            for (int i = 0; i < 16 && ptr + i < end; i++) {
                if (ptr[i] == first_byte && ptr[i + offset] == last_byte) {
                    if (needle_len <= 2 || memcmp(ptr + i + 1, needle + 1, needle_len - 2) == 0) {
                        return ptr + i;
                    }
                }
            }
        }

        /* Check if any candidates in second chunk */
        uint64x2_t cand64_1 = vreinterpretq_u64_u8(candidates1);
        if (vgetq_lane_u64(cand64_1, 0) || vgetq_lane_u64(cand64_1, 1)) {
            for (int i = 0; i < 16 && ptr + 16 + i < end; i++) {
                if (ptr[16 + i] == first_byte && ptr[16 + i + offset] == last_byte) {
                    if (needle_len <= 2 || memcmp(ptr + 16 + i + 1, needle + 1, needle_len - 2) == 0) {
                        return ptr + 16 + i;
                    }
                }
            }
        }

        ptr += 32;
    }

    /* Handle remaining 16 bytes */
    while (ptr + 16 <= end) {
        uint8x16_t chunk_first = vld1q_u8(ptr);
        uint8x16_t chunk_last = vld1q_u8(ptr + offset);
        uint8x16_t eq_first = vceqq_u8(chunk_first, first_vec);
        uint8x16_t eq_last = vceqq_u8(chunk_last, last_vec);
        uint8x16_t candidates = vandq_u8(eq_first, eq_last);

        uint64x2_t cand64 = vreinterpretq_u64_u8(candidates);
        if (vgetq_lane_u64(cand64, 0) || vgetq_lane_u64(cand64, 1)) {
            for (int i = 0; i < 16 && ptr + i < end; i++) {
                if (ptr[i] == first_byte && ptr[i + offset] == last_byte) {
                    if (needle_len <= 2 || memcmp(ptr + i + 1, needle + 1, needle_len - 2) == 0) {
                        return ptr + i;
                    }
                }
            }
        }
        ptr += 16;
    }

    /* Scalar tail */
    while (ptr < end) {
        if (*ptr == first_byte && ptr[offset] == last_byte) {
            if (needle_len <= 2 || memcmp(ptr + 1, needle + 1, needle_len - 2) == 0) {
                return ptr;
            }
        }
        ptr++;
    }

    return NULL;
}

#endif /* USE_NEON */

/*
 * Scalar fallback implementations
 */
static inline const uint8_t* memchr2_scalar(uint8_t n1, uint8_t n2,
                                             const uint8_t* haystack, size_t len) {
    const uint8_t* end = haystack + len;

    for (const uint8_t* ptr = haystack; ptr < end; ptr++) {
        if (*ptr == n1 || *ptr == n2) return ptr;
    }

    return NULL;
}

static inline const uint8_t* memchr3_scalar(uint8_t n1, uint8_t n2, uint8_t n3,
                                             const uint8_t* haystack, size_t len) {
    const uint8_t* end = haystack + len;

    for (const uint8_t* ptr = haystack; ptr < end; ptr++) {
        if (*ptr == n1 || *ptr == n2 || *ptr == n3) return ptr;
    }

    return NULL;
}

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
 * Scalar memmem with Boyer-Moore-Horspool
 */
static inline const uint8_t* memmem_scalar(const uint8_t* needle, size_t needle_len,
                                            const uint8_t* haystack, size_t haystack_len) {
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
}

/*
 * Public memchr2 function - dispatches to best implementation
 */
const uint8_t* memchr2_find(uint8_t n1, uint8_t n2, const uint8_t* haystack, size_t len) {
#ifdef USE_X86_64
    ENSURE_CPU_DETECTED();
    if (has_avx2) {
        return memchr2_avx2(n1, n2, haystack, len);
    }
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
#ifdef USE_X86_64
    ENSURE_CPU_DETECTED();
    if (has_avx2) {
        return memchr3_avx2(n1, n2, n3, haystack, len);
    }
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
#ifdef USE_X86_64
    ENSURE_CPU_DETECTED();
    if (has_avx2) {
        return memrchr2_avx2(n1, n2, haystack, len);
    }
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
#ifdef USE_X86_64
    ENSURE_CPU_DETECTED();
    if (has_avx2) {
        return memrchr3_avx2(n1, n2, n3, haystack, len);
    }
    return memrchr3_sse2(n1, n2, n3, haystack, len);
#elif defined(USE_NEON)
    return memrchr3_neon(n1, n2, n3, haystack, len);
#else
    return memrchr3_scalar(n1, n2, n3, haystack, len);
#endif
}

/*
 * Substring search - forward
 * Uses SIMD-accelerated prefiltering with Two-Way style algorithm
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

    /* For very short needles (2-3 bytes), use simple SIMD search */
    if (needle_len == 2) {
        const uint8_t* ptr = haystack;
        const uint8_t* end = haystack + haystack_len - 1;
        while (ptr < end) {
            ptr = memchr_find(needle[0], ptr, end - ptr);
            if (ptr == NULL) return NULL;
            if (ptr[1] == needle[1]) return ptr;
            ptr++;
        }
        return NULL;
    }

#ifdef USE_X86_64
    ENSURE_CPU_DETECTED();
    if (has_avx2) {
        return memmem_avx2(needle, needle_len, haystack, haystack_len);
    }
    return memmem_sse2(needle, needle_len, haystack, haystack_len);
#elif defined(USE_NEON)
    return memmem_neon(needle, needle_len, haystack, haystack_len);
#else
    return memmem_scalar(needle, needle_len, haystack, haystack_len);
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

    /* Search from end using SIMD prefiltering */
    const uint8_t* ptr = haystack + haystack_len - needle_len;
    uint8_t last_byte = needle[needle_len - 1];
    uint8_t first_byte = needle[0];

    while (ptr >= haystack) {
        /* Quick check on first and last bytes before full comparison */
        if (*ptr == first_byte && ptr[needle_len - 1] == last_byte) {
            if (memcmp(ptr, needle, needle_len) == 0) {
                return ptr;
            }
        }
        ptr--;
    }

    return NULL;
}
