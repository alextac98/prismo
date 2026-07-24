#include <chrono>
#include <cmath>
#include <cstdint>
#include <string>
#include <thread>

#include "PluginSession.hpp"

namespace {

std::uint64_t unix_timestamp_ns() {
  auto now = std::chrono::system_clock::now().time_since_epoch();
  return static_cast<std::uint64_t>(
      std::chrono::duration_cast<std::chrono::nanoseconds>(now).count());
}

}  // namespace

int main() {
  auto plugin = PluginSession::from_stdio();
  if (!plugin) {
    return 1;
  }

  if (!plugin->send_hello("0.1.0", "cpp").ok().value_or(false)) {
    return 1;
  }

  if (!plugin
           ->declare_channel("cpp.system.heartbeat", "Heartbeat", "",
                             "Heartbeat counter")
           .ok()
           .value_or(false)) {
    return 1;
  }
  if (!plugin
           ->declare_channel("cpp.system.sine", "Sine", "",
                             "Sine-wave demo value")
           .ok()
           .value_or(false)) {
    return 1;
  }
  if (!plugin
           ->declare_channel("cpp.system.online", "Online", "",
                             "C++ plugin health flag")
           .ok()
           .value_or(false)) {
    return 1;
  }
  if (!plugin
           ->declare_channel("cpp.system.mode", "Mode", "",
                             "C++ plugin operating mode")
           .ok()
           .value_or(false)) {
    return 1;
  }

  std::uint64_t sequence = 0;
  while (true) {
    sequence += 1;
    const auto timestamp = unix_timestamp_ns();
    const auto phase = static_cast<double>(sequence) / 8.0;

    if (!plugin
             ->send_integer_sample("cpp.system.heartbeat", timestamp, sequence,
                                   static_cast<std::int64_t>(sequence))
             .ok()
             .value_or(false)) {
      return 1;
    }
    if (!plugin->send_float_sample("cpp.system.sine", timestamp, sequence,
                                   std::sin(phase))
             .ok()
             .value_or(false)) {
      return 1;
    }
    if (!plugin->send_bool_sample("cpp.system.online", timestamp, sequence, true)
             .ok()
             .value_or(false)) {
      return 1;
    }
    const auto mode = static_cast<std::int64_t>(sequence % 3);
    const char* mode_name =
        mode == 0 ? "IDLE" : (mode == 1 ? "ACTIVE" : "DIAGNOSTIC");
    if (!plugin
             ->send_enum_sample("cpp.system.mode", timestamp, sequence, mode,
                                mode_name)
             .ok()
             .value_or(false)) {
      return 1;
    }
    if (!plugin->send_health(sequence, 0, "").ok().value_or(false)) {
      return 1;
    }

    std::this_thread::sleep_for(std::chrono::milliseconds(200));
  }
}
