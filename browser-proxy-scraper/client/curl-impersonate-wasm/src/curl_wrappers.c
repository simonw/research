/**
 * Non-variadic wrappers for curl_easy_setopt and curl_easy_getinfo.
 *
 * curl_easy_setopt is variadic: CURLcode curl_easy_setopt(CURL*, CURLoption, ...)
 * Calling variadic C functions from JavaScript via Emscripten's cwrap breaks
 * va_arg for pointer arguments (string options cause memory access out of bounds).
 * These fixed-signature wrappers keep the variadic handling entirely within
 * WASM C code, allowing JS to call non-variadic functions safely.
 */

#include <emscripten.h>
#include <curl/curl.h>

EMSCRIPTEN_KEEPALIVE
CURLcode curl_setopt_string(CURL *handle, CURLoption option, const char *value) {
    return curl_easy_setopt(handle, option, value);
}

EMSCRIPTEN_KEEPALIVE
CURLcode curl_setopt_long(CURL *handle, CURLoption option, long value) {
    return curl_easy_setopt(handle, option, value);
}

EMSCRIPTEN_KEEPALIVE
CURLcode curl_setopt_ptr(CURL *handle, CURLoption option, void *value) {
    return curl_easy_setopt(handle, option, value);
}

EMSCRIPTEN_KEEPALIVE
CURLcode curl_setopt_cb(CURL *handle, CURLoption option, void *callback) {
    return curl_easy_setopt(handle, option, callback);
}

EMSCRIPTEN_KEEPALIVE
CURLcode curl_getinfo_long(CURL *handle, CURLINFO info, long *value) {
    return curl_easy_getinfo(handle, info, value);
}

/**
 * Replicate curl_easy_impersonate("chrome116", default_headers=0) without
 * CURLOPT_SSL_CERT_COMPRESSION. The brotli cert decompression callback
 * (DecompressBrotliCert) causes WASM indirect call type mismatches because
 * BoringSSL invokes it through a function pointer table with Asyncify-altered
 * type signatures.
 */
EMSCRIPTEN_KEEPALIVE
CURLcode curl_impersonate_chrome116(CURL *handle) {
    CURLcode rc;

    /* HTTP/2 */
    rc = curl_easy_setopt(handle, CURLOPT_HTTP_VERSION, CURL_HTTP_VERSION_2_0);
    if (rc) return rc;

    /* TLS 1.2+ */
    rc = curl_easy_setopt(handle, CURLOPT_SSLVERSION,
        CURL_SSLVERSION_TLSv1_2 | CURL_SSLVERSION_MAX_DEFAULT);
    if (rc) return rc;

    /* Chrome 116 cipher suite order */
    rc = curl_easy_setopt(handle, CURLOPT_SSL_CIPHER_LIST,
        "TLS_AES_128_GCM_SHA256,"
        "TLS_AES_256_GCM_SHA384,"
        "TLS_CHACHA20_POLY1305_SHA256,"
        "ECDHE-ECDSA-AES128-GCM-SHA256,"
        "ECDHE-RSA-AES128-GCM-SHA256,"
        "ECDHE-ECDSA-AES256-GCM-SHA384,"
        "ECDHE-RSA-AES256-GCM-SHA384,"
        "ECDHE-ECDSA-CHACHA20-POLY1305,"
        "ECDHE-RSA-CHACHA20-POLY1305,"
        "ECDHE-RSA-AES128-SHA,"
        "ECDHE-RSA-AES256-SHA,"
        "AES128-GCM-SHA256,"
        "AES256-GCM-SHA384,"
        "AES128-SHA,"
        "AES256-SHA");
    if (rc) return rc;

    /* NPN disabled, ALPN enabled */
    curl_easy_setopt(handle, CURLOPT_SSL_ENABLE_NPN, 0L);
    curl_easy_setopt(handle, CURLOPT_SSL_ENABLE_ALPN, 1L);

    /* ALPS (Application-Layer Protocol Settings) */
    curl_easy_setopt(handle, CURLOPT_SSL_ENABLE_ALPS, 1L);

    /* Session tickets */
    curl_easy_setopt(handle, CURLOPT_SSL_ENABLE_TICKET, 1L);

    /* TLS extension permutation (chrome110+) */
    curl_easy_setopt(handle, CURLOPT_SSL_PERMUTE_EXTENSIONS, 1L);

    /* Skip CURLOPT_SSL_CERT_COMPRESSION — causes WASM indirect call crash */

    /* HTTP/2: no server push */
    curl_easy_setopt(handle, CURLOPT_HTTP2_NO_SERVER_PUSH, 1L);

    /* Accept-Encoding (auto-negotiate) */
    curl_easy_setopt(handle, CURLOPT_ACCEPT_ENCODING, "");

    return CURLE_OK;
}
