#ifndef SRC_NODE_API_H_
#define SRC_NODE_API_H_

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct napi_env__* napi_env;
typedef struct napi_value__* napi_value;
typedef struct napi_ref__* napi_ref;
typedef struct napi_handle_scope__* napi_handle_scope;
typedef struct napi_escapable_handle_scope__* napi_escapable_handle_scope;
typedef struct napi_callback_info__* napi_callback_info;
typedef struct napi_async_context__* napi_async_context;
typedef struct napi_async_work__* napi_async_work;

typedef enum {
  napi_ok,
  napi_invalid_arg,
  napi_object_expected,
  napi_string_expected,
  napi_name_expected,
  napi_function_expected,
  napi_number_expected,
  napi_boolean_expected,
  napi_array_expected,
  napi_generic_failure,
  napi_pending_exception,
  napi_cancelled,
  napi_escape_called_twice,
  napi_handle_scope_mismatch,
  napi_callback_scope_mismatch,
  napi_queue_full,
  napi_closing,
  napi_bigint_expected,
  napi_date_expected,
  napi_arraybuffer_expected,
  napi_detachable_arraybuffer_expected,
  napi_would_deadlock
} napi_status;

typedef napi_value (*napi_callback)(napi_env env, napi_callback_info info);

#if defined(_WIN32)
#define NAPI_MODULE_EXPORT __declspec(dllexport)
#else
#define NAPI_MODULE_EXPORT __attribute__((visibility("default")))
#endif

#define NAPI_MODULE_INIT()                                             \
  NAPI_MODULE_EXPORT napi_value                                        \
  napi_register_module_v1(napi_env env, napi_value exports)

napi_status napi_get_cb_info(napi_env env,
                             napi_callback_info cbinfo,
                             size_t* argc,
                             napi_value* argv,
                             napi_value* this_arg,
                             void** data);

napi_status napi_get_buffer_info(napi_env env,
                                 napi_value value,
                                 void** data,
                                 size_t* length);

napi_status napi_create_buffer(napi_env env,
                               size_t size,
                               void** data,
                               napi_value* result);

napi_status napi_create_buffer_copy(napi_env env,
                                    size_t length,
                                    const void* data,
                                    void** result_data,
                                    napi_value* result);

napi_status napi_create_uint32(napi_env env, uint32_t value, napi_value* result);
napi_status napi_get_value_uint32(napi_env env, napi_value value, uint32_t* result);
napi_status napi_create_int64(napi_env env, int64_t value, napi_value* result);
napi_status napi_create_bigint_uint64(napi_env env, uint64_t value, napi_value* result);
napi_status napi_create_object(napi_env env, napi_value* result);
napi_status napi_set_named_property(napi_env env,
                                    napi_value object,
                                    const char* utf8name,
                                    napi_value value);
napi_status napi_create_array_with_length(napi_env env, size_t length, napi_value* result);
napi_status napi_set_element(napi_env env, napi_value object, uint32_t index, napi_value value);
napi_status napi_get_undefined(napi_env env, napi_value* result);
napi_status napi_throw_error(napi_env env, const char* code, const char* msg);
napi_status napi_create_function(napi_env env,
                                 const char* utf8name,
                                 size_t length,
                                 napi_callback cb,
                                 void* data,
                                 napi_value* result);

#ifdef __cplusplus
}
#endif

#endif  // SRC_NODE_API_H_
