// `host` module: lets sandboxed Python call functions registered by the host.
//
//   import host
//   host.call("add", 1, 2)   -> 3
//
// Arguments are JSON-encoded with MicroPython's json module and handed to the
// host with host_call(); the host answers with a JSON string that the guest
// fetches into a malloc'd buffer with host_take() and decodes.
#include <stdlib.h>
#include <string.h>

#include "py/runtime.h"
#include "py/objstr.h"

__attribute__((import_module("env"), import_name("host_call")))
int host_call(const char *name, size_t name_len, const char *json, size_t json_len);

__attribute__((import_module("env"), import_name("host_take")))
void host_take(char *buf, size_t len);

extern const mp_obj_module_t mp_module_json;

static mp_obj_t json_method(qstr name, mp_obj_t arg) {
    mp_obj_t fn = mp_load_attr(MP_OBJ_FROM_PTR(&mp_module_json), name);
    return mp_call_function_1(fn, arg);
}

static mp_obj_t host_call_py(size_t n_args, const mp_obj_t *args) {
    size_t name_len;
    const char *name = mp_obj_str_get_data(args[0], &name_len);

    mp_obj_t arg_tuple = mp_obj_new_tuple(n_args - 1, args + 1);
    mp_obj_t encoded = json_method(MP_QSTR_dumps, arg_tuple);
    size_t json_len;
    const char *json = mp_obj_str_get_data(encoded, &json_len);

    int ret = host_call(name, name_len, json, json_len);
    size_t out_len = (size_t)(ret < 0 ? -ret : ret);
    char *buf = malloc(out_len + 1);
    if (buf == NULL) {
        mp_raise_msg(&mp_type_MemoryError, MP_ERROR_TEXT("host result too large"));
    }
    host_take(buf, out_len);
    buf[out_len] = '\0';
    if (ret < 0) {
        mp_obj_t msg = mp_obj_new_str(buf, out_len);
        free(buf);
        nlr_raise(mp_obj_new_exception_arg1(&mp_type_RuntimeError, msg));
    }
    mp_obj_t result_str = mp_obj_new_str(buf, out_len);
    free(buf);
    return json_method(MP_QSTR_loads, result_str);
}
static MP_DEFINE_CONST_FUN_OBJ_VAR_BETWEEN(host_call_obj, 1, MP_OBJ_FUN_ARGS_MAX, host_call_py);

static const mp_rom_map_elem_t host_module_globals_table[] = {
    { MP_ROM_QSTR(MP_QSTR___name__), MP_ROM_QSTR(MP_QSTR_host) },
    { MP_ROM_QSTR(MP_QSTR_call), MP_ROM_PTR(&host_call_obj) },
};
static MP_DEFINE_CONST_DICT(host_module_globals, host_module_globals_table);

const mp_obj_module_t host_module = {
    .base = { &mp_type_module },
    .globals = (mp_obj_dict_t *)&host_module_globals,
};

MP_REGISTER_MODULE(MP_QSTR_host, host_module);
