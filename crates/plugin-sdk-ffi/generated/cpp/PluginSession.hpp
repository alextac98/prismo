#ifndef PluginSession_HPP
#define PluginSession_HPP

#include "PluginSession.d.hpp"

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
    extern "C" {
    
    diplomat::capi::PluginSession* prismo_PluginSession_from_stdio(void);
    
    void prismo_PluginSession_plugin_id(const diplomat::capi::PluginSession* self, diplomat::capi::DiplomatWrite* write);
    
    void prismo_PluginSession_config_json(const diplomat::capi::PluginSession* self, diplomat::capi::DiplomatWrite* write);
    
    bool prismo_PluginSession_send_hello(diplomat::capi::PluginSession* self, diplomat::capi::DiplomatStringView plugin_version, diplomat::capi::DiplomatStringView language);
    
    bool prismo_PluginSession_declare_channel(diplomat::capi::PluginSession* self, diplomat::capi::DiplomatStringView channel_path, diplomat::capi::DiplomatStringView display_name, diplomat::capi::DiplomatStringView unit, diplomat::capi::DiplomatStringView description);
    
    bool prismo_PluginSession_send_bool_sample(diplomat::capi::PluginSession* self, diplomat::capi::DiplomatStringView channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, bool value);
    
    bool prismo_PluginSession_send_integer_sample(diplomat::capi::PluginSession* self, diplomat::capi::DiplomatStringView channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, int64_t value);
    
    bool prismo_PluginSession_send_float_sample(diplomat::capi::PluginSession* self, diplomat::capi::DiplomatStringView channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, double value);
    
    bool prismo_PluginSession_send_text_sample(diplomat::capi::PluginSession* self, diplomat::capi::DiplomatStringView channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, diplomat::capi::DiplomatStringView value);
    
    bool prismo_PluginSession_send_bytes_sample(diplomat::capi::PluginSession* self, diplomat::capi::DiplomatStringView channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, diplomat::capi::DiplomatU8View value);
    
    bool prismo_PluginSession_send_health(diplomat::capi::PluginSession* self, uint64_t emitted_updates, uint64_t dropped_updates, diplomat::capi::DiplomatStringView last_error);
    
    bool prismo_PluginSession_send_log(diplomat::capi::PluginSession* self, diplomat::capi::DiplomatStringView level, diplomat::capi::DiplomatStringView message);
    
    
    void prismo_PluginSession_destroy(PluginSession* self);
    
    } // extern "C"
} // namespace capi
} // namespace

inline std::unique_ptr<PluginSession> PluginSession::from_stdio() {
  auto result = diplomat::capi::prismo_PluginSession_from_stdio();
  return std::unique_ptr<PluginSession>(PluginSession::FromFFI(result));
}

inline std::string PluginSession::plugin_id() const {
  std::string output;
  diplomat::capi::DiplomatWrite write = diplomat::WriteFromString(output);
  diplomat::capi::prismo_PluginSession_plugin_id(this->AsFFI(),
    &write);
  return output;
}

inline std::string PluginSession::config_json() const {
  std::string output;
  diplomat::capi::DiplomatWrite write = diplomat::WriteFromString(output);
  diplomat::capi::prismo_PluginSession_config_json(this->AsFFI(),
    &write);
  return output;
}

inline diplomat::result<bool, diplomat::Utf8Error> PluginSession::send_hello(std::string_view plugin_version, std::string_view language) {
  if (!diplomat::capi::diplomat_is_str(plugin_version.data(), plugin_version.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  if (!diplomat::capi::diplomat_is_str(language.data(), language.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  auto result = diplomat::capi::prismo_PluginSession_send_hello(this->AsFFI(),
    {plugin_version.data(), plugin_version.size()},
    {language.data(), language.size()});
  return diplomat::Ok<bool>(result);
}

inline diplomat::result<bool, diplomat::Utf8Error> PluginSession::declare_channel(std::string_view channel_path, std::string_view display_name, std::string_view unit, std::string_view description) {
  if (!diplomat::capi::diplomat_is_str(channel_path.data(), channel_path.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  if (!diplomat::capi::diplomat_is_str(display_name.data(), display_name.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  if (!diplomat::capi::diplomat_is_str(unit.data(), unit.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  if (!diplomat::capi::diplomat_is_str(description.data(), description.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  auto result = diplomat::capi::prismo_PluginSession_declare_channel(this->AsFFI(),
    {channel_path.data(), channel_path.size()},
    {display_name.data(), display_name.size()},
    {unit.data(), unit.size()},
    {description.data(), description.size()});
  return diplomat::Ok<bool>(result);
}

inline diplomat::result<bool, diplomat::Utf8Error> PluginSession::send_bool_sample(std::string_view channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, bool value) {
  if (!diplomat::capi::diplomat_is_str(channel_path.data(), channel_path.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  auto result = diplomat::capi::prismo_PluginSession_send_bool_sample(this->AsFFI(),
    {channel_path.data(), channel_path.size()},
    timestamp_unix_ns,
    sequence,
    value);
  return diplomat::Ok<bool>(result);
}

inline diplomat::result<bool, diplomat::Utf8Error> PluginSession::send_integer_sample(std::string_view channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, int64_t value) {
  if (!diplomat::capi::diplomat_is_str(channel_path.data(), channel_path.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  auto result = diplomat::capi::prismo_PluginSession_send_integer_sample(this->AsFFI(),
    {channel_path.data(), channel_path.size()},
    timestamp_unix_ns,
    sequence,
    value);
  return diplomat::Ok<bool>(result);
}

inline diplomat::result<bool, diplomat::Utf8Error> PluginSession::send_float_sample(std::string_view channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, double value) {
  if (!diplomat::capi::diplomat_is_str(channel_path.data(), channel_path.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  auto result = diplomat::capi::prismo_PluginSession_send_float_sample(this->AsFFI(),
    {channel_path.data(), channel_path.size()},
    timestamp_unix_ns,
    sequence,
    value);
  return diplomat::Ok<bool>(result);
}

inline diplomat::result<bool, diplomat::Utf8Error> PluginSession::send_text_sample(std::string_view channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, std::string_view value) {
  if (!diplomat::capi::diplomat_is_str(channel_path.data(), channel_path.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  if (!diplomat::capi::diplomat_is_str(value.data(), value.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  auto result = diplomat::capi::prismo_PluginSession_send_text_sample(this->AsFFI(),
    {channel_path.data(), channel_path.size()},
    timestamp_unix_ns,
    sequence,
    {value.data(), value.size()});
  return diplomat::Ok<bool>(result);
}

inline diplomat::result<bool, diplomat::Utf8Error> PluginSession::send_bytes_sample(std::string_view channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, diplomat::span<const uint8_t> value) {
  if (!diplomat::capi::diplomat_is_str(channel_path.data(), channel_path.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  auto result = diplomat::capi::prismo_PluginSession_send_bytes_sample(this->AsFFI(),
    {channel_path.data(), channel_path.size()},
    timestamp_unix_ns,
    sequence,
    {value.data(), value.size()});
  return diplomat::Ok<bool>(result);
}

inline diplomat::result<bool, diplomat::Utf8Error> PluginSession::send_health(uint64_t emitted_updates, uint64_t dropped_updates, std::string_view last_error) {
  if (!diplomat::capi::diplomat_is_str(last_error.data(), last_error.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  auto result = diplomat::capi::prismo_PluginSession_send_health(this->AsFFI(),
    emitted_updates,
    dropped_updates,
    {last_error.data(), last_error.size()});
  return diplomat::Ok<bool>(result);
}

inline diplomat::result<bool, diplomat::Utf8Error> PluginSession::send_log(std::string_view level, std::string_view message) {
  if (!diplomat::capi::diplomat_is_str(level.data(), level.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  if (!diplomat::capi::diplomat_is_str(message.data(), message.size())) {
    return diplomat::Err<diplomat::Utf8Error>();
  }
  auto result = diplomat::capi::prismo_PluginSession_send_log(this->AsFFI(),
    {level.data(), level.size()},
    {message.data(), message.size()});
  return diplomat::Ok<bool>(result);
}

inline const diplomat::capi::PluginSession* PluginSession::AsFFI() const {
  return reinterpret_cast<const diplomat::capi::PluginSession*>(this);
}

inline diplomat::capi::PluginSession* PluginSession::AsFFI() {
  return reinterpret_cast<diplomat::capi::PluginSession*>(this);
}

inline const PluginSession* PluginSession::FromFFI(const diplomat::capi::PluginSession* ptr) {
  return reinterpret_cast<const PluginSession*>(ptr);
}

inline PluginSession* PluginSession::FromFFI(diplomat::capi::PluginSession* ptr) {
  return reinterpret_cast<PluginSession*>(ptr);
}

inline void PluginSession::operator delete(void* ptr) {
  diplomat::capi::prismo_PluginSession_destroy(reinterpret_cast<diplomat::capi::PluginSession*>(ptr));
}


#endif // PluginSession_HPP
