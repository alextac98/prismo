#ifndef PluginSession_D_HPP
#define PluginSession_D_HPP

#include <stdio.h>
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <memory>
#include <functional>
#include <optional>
#include "diplomat_runtime.hpp"


namespace diplomat {
namespace capi {
    struct PluginSession;
} // namespace capi
} // namespace

class PluginSession {
public:

  inline static std::unique_ptr<PluginSession> from_stdio();

  inline std::string plugin_id() const;

  inline std::string config_json() const;

  inline diplomat::result<bool, diplomat::Utf8Error> send_hello(std::string_view plugin_version, std::string_view language);

  inline diplomat::result<bool, diplomat::Utf8Error> declare_channel(std::string_view channel_path, std::string_view display_name, std::string_view unit, std::string_view description);

  inline diplomat::result<bool, diplomat::Utf8Error> send_bool_sample(std::string_view channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, bool value);

  inline diplomat::result<bool, diplomat::Utf8Error> send_integer_sample(std::string_view channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, int64_t value);

  inline diplomat::result<bool, diplomat::Utf8Error> send_float_sample(std::string_view channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, double value);

  inline diplomat::result<bool, diplomat::Utf8Error> send_text_sample(std::string_view channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, std::string_view value);

  inline diplomat::result<bool, diplomat::Utf8Error> send_bytes_sample(std::string_view channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, diplomat::span<const uint8_t> value);

  inline diplomat::result<bool, diplomat::Utf8Error> send_health(uint64_t emitted_updates, uint64_t dropped_updates, std::string_view last_error);

  inline diplomat::result<bool, diplomat::Utf8Error> send_log(std::string_view level, std::string_view message);

  inline const diplomat::capi::PluginSession* AsFFI() const;
  inline diplomat::capi::PluginSession* AsFFI();
  inline static const PluginSession* FromFFI(const diplomat::capi::PluginSession* ptr);
  inline static PluginSession* FromFFI(diplomat::capi::PluginSession* ptr);
  inline static void operator delete(void* ptr);
private:
  PluginSession() = delete;
  PluginSession(const PluginSession&) = delete;
  PluginSession(PluginSession&&) noexcept = delete;
  PluginSession operator=(const PluginSession&) = delete;
  PluginSession operator=(PluginSession&&) noexcept = delete;
  static void operator delete[](void*, size_t) = delete;
};


#endif // PluginSession_D_HPP
