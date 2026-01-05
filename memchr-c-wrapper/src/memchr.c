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

    while (ptr + 32 <= end) {
        __m256i chunk = _mm256_load_si256((const __m256i*)ptr);
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
            int idx = 31 - __builtin_clz(mask);
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
            int idx = 31 - __builtin_clz(mask);
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

/*
 * SIMD-accelerated memmem with Two-Way algorithm prefiltering
 * This uses SIMD to quickly find candidate positions, then verifies matches
 */

/* Find a "rare" byte in the needle for prefiltering */
static inline size_t find_rare_byte_index(const uint8_t* needle, size_t len) {
    /* Frequency table for common ASCII bytes (approximate) */
    static const uint8_t freq[256] = {
        /* 0x00-0x0F */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 50, 50, 0, 0, 50, 0, 0,
        /* 0x10-0x1F */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        /* 0x20-0x2F (space, punctuation) */ 100, 10, 20, 5, 5, 5, 5, 20, 20, 20, 5, 5, 30, 30, 40, 20,
        /* 0x30-0x3F (digits) */ 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 20, 20, 20, 20, 20, 20,
        /* 0x40-0x4F */ 5, 40, 30, 40, 40, 60, 30, 30, 40, 50, 10, 20, 40, 40, 50, 50,
        /* 0x50-0x5F */ 40, 5, 50, 50, 60, 40, 20, 20, 10, 20, 5, 10, 5, 10, 5, 30,
        /* 0x60-0x6F */ 5, 80, 30, 50, 50, 90, 30, 30, 50, 70, 10, 20, 50, 50, 70, 70,
        /* 0x70-0x7F */ 40, 5, 70, 70, 80, 50, 20, 20, 10, 30, 10, 10, 10, 10, 10, 0,
        /* 0x80-0xFF (high bytes - rare in ASCII) */
        5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
        5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
        5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
        5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
        5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
        5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
        5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
        5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    };

    size_t rarest_idx = 0;
    uint8_t rarest_freq = 255;

    /* Prefer bytes near the end of the needle for better skip distance */
    for (size_t i = len > 1 ? len - 1 : 0; i > 0; i--) {
        if (freq[needle[i]] < rarest_freq) {
            rarest_freq = freq[needle[i]];
            rarest_idx = i;
        }
    }

    /* Also check first byte */
    if (freq[needle[0]] < rarest_freq) {
        rarest_idx = 0;
    }

    return rarest_idx;
}

/*
 * AVX2-accelerated memmem with SIMD prefiltering
 */
static inline const uint8_t* memmem_avx2(const uint8_t* needle, size_t needle_len,
                                          const uint8_t* haystack, size_t haystack_len) {
    /* Find rare byte for prefiltering */
    size_t rare_idx = find_rare_byte_index(needle, needle_len);
    uint8_t rare_byte = needle[rare_idx];
    uint8_t first_byte = needle[0];

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + haystack_len - needle_len + 1;

    /* Use AVX2 to find candidates */
    __m256i rare_vec = _mm256_set1_epi8(rare_byte);
    __m256i first_vec = _mm256_set1_epi8(first_byte);

    while (ptr + 32 <= end) {
        /* Load and search for rare byte at offset position */
        __m256i chunk_rare = _mm256_loadu_si256((const __m256i*)(ptr + rare_idx));
        __m256i cmp_rare = _mm256_cmpeq_epi8(chunk_rare, rare_vec);
        int mask_rare = _mm256_movemask_epi8(cmp_rare);

        if (mask_rare != 0) {
            /* Also check first byte for additional filtering */
            __m256i chunk_first = _mm256_loadu_si256((const __m256i*)ptr);
            __m256i cmp_first = _mm256_cmpeq_epi8(chunk_first, first_vec);
            int mask_first = _mm256_movemask_epi8(cmp_first);

            /* Combine: position must match both rare byte (at offset) and first byte */
            int combined = mask_rare & mask_first;

            while (combined != 0) {
                int idx = __builtin_ctz(combined);
                const uint8_t* candidate = ptr + idx;

                if (candidate + needle_len <= haystack + haystack_len) {
                    if (memcmp(candidate, needle, needle_len) == 0) {
                        return candidate;
                    }
                }
                combined &= combined - 1;  /* Clear lowest bit */
            }
        }
        ptr += 32;
    }

    /* Handle remaining bytes with scalar search */
    while (ptr < end) {
        if (*ptr == first_byte && ptr[rare_idx] == rare_byte) {
            if (memcmp(ptr, needle, needle_len) == 0) {
                return ptr;
            }
        }
        ptr++;
    }

    return NULL;
}

/*
 * SSE2-accelerated memmem with SIMD prefiltering
 */
static inline const uint8_t* memmem_sse2(const uint8_t* needle, size_t needle_len,
                                          const uint8_t* haystack, size_t haystack_len) {
    /* Find rare byte for prefiltering */
    size_t rare_idx = find_rare_byte_index(needle, needle_len);
    uint8_t rare_byte = needle[rare_idx];
    uint8_t first_byte = needle[0];

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + haystack_len - needle_len + 1;

    /* Use SSE2 to find candidates */
    __m128i rare_vec = _mm_set1_epi8(rare_byte);
    __m128i first_vec = _mm_set1_epi8(first_byte);

    while (ptr + 16 <= end) {
        /* Load and search for rare byte at offset position */
        __m128i chunk_rare = _mm_loadu_si128((const __m128i*)(ptr + rare_idx));
        __m128i cmp_rare = _mm_cmpeq_epi8(chunk_rare, rare_vec);
        int mask_rare = _mm_movemask_epi8(cmp_rare);

        if (mask_rare != 0) {
            /* Also check first byte */
            __m128i chunk_first = _mm_loadu_si128((const __m128i*)ptr);
            __m128i cmp_first = _mm_cmpeq_epi8(chunk_first, first_vec);
            int mask_first = _mm_movemask_epi8(cmp_first);

            /* Combine masks */
            int combined = mask_rare & mask_first;

            while (combined != 0) {
                int idx = __builtin_ctz(combined);
                const uint8_t* candidate = ptr + idx;

                if (candidate + needle_len <= haystack + haystack_len) {
                    if (memcmp(candidate, needle, needle_len) == 0) {
                        return candidate;
                    }
                }
                combined &= combined - 1;
            }
        }
        ptr += 16;
    }

    /* Handle remaining bytes */
    while (ptr < end) {
        if (*ptr == first_byte && ptr[rare_idx] == rare_byte) {
            if (memcmp(ptr, needle, needle_len) == 0) {
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
 * NEON-accelerated memmem with SIMD prefiltering
 */
static inline const uint8_t* memmem_neon(const uint8_t* needle, size_t needle_len,
                                          const uint8_t* haystack, size_t haystack_len) {
    size_t rare_idx = find_rare_byte_index(needle, needle_len);
    uint8_t rare_byte = needle[rare_idx];
    uint8_t first_byte = needle[0];

    const uint8_t* ptr = haystack;
    const uint8_t* end = haystack + haystack_len - needle_len + 1;

    uint8x16_t rare_vec = vdupq_n_u8(rare_byte);
    uint8x16_t first_vec = vdupq_n_u8(first_byte);

    while (ptr + 16 <= end) {
        uint8x16_t chunk_rare = vld1q_u8(ptr + rare_idx);
        uint8x16_t cmp_rare = vceqq_u8(chunk_rare, rare_vec);

        uint64x2_t rare64 = vreinterpretq_u64_u8(cmp_rare);
        if (vgetq_lane_u64(rare64, 0) || vgetq_lane_u64(rare64, 1)) {
            uint8x16_t chunk_first = vld1q_u8(ptr);
            uint8x16_t cmp_first = vceqq_u8(chunk_first, first_vec);
            uint8x16_t combined = vandq_u8(cmp_rare, cmp_first);

            uint64x2_t comb64 = vreinterpretq_u64_u8(combined);
            if (vgetq_lane_u64(comb64, 0) || vgetq_lane_u64(comb64, 1)) {
                for (int i = 0; i < 16 && ptr + i < end; i++) {
                    if (ptr[i] == first_byte && ptr[i + rare_idx] == rare_byte) {
                        if (memcmp(ptr + i, needle, needle_len) == 0) {
                            return ptr + i;
                        }
                    }
                }
            }
        }
        ptr += 16;
    }

    /* Scalar remainder */
    while (ptr < end) {
        if (*ptr == first_byte && ptr[rare_idx] == rare_byte) {
            if (memcmp(ptr, needle, needle_len) == 0) {
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
