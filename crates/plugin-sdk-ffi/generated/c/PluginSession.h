#ifndef PluginSession_H
#define PluginSession_H

#include <stdio.h>
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include "diplomat_runtime.h"


#include "PluginSession.d.h"






PluginSession* prismo_PluginSession_from_stdio(void);

void prismo_PluginSession_plugin_id(const PluginSession* self, DiplomatWrite* write);

void prismo_PluginSession_config_json(const PluginSession* self, DiplomatWrite* write);

bool prismo_PluginSession_send_hello(PluginSession* self, DiplomatStringView plugin_version, DiplomatStringView language);

bool prismo_PluginSession_declare_channel(PluginSession* self, DiplomatStringView channel_path, DiplomatStringView display_name, DiplomatStringView unit, DiplomatStringView description);

bool prismo_PluginSession_send_bool_sample(PluginSession* self, DiplomatStringView channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, bool value);

bool prismo_PluginSession_send_integer_sample(PluginSession* self, DiplomatStringView channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, int64_t value);

bool prismo_PluginSession_send_float_sample(PluginSession* self, DiplomatStringView channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, double value);

bool prismo_PluginSession_send_text_sample(PluginSession* self, DiplomatStringView channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, DiplomatStringView value);

bool prismo_PluginSession_send_bytes_sample(PluginSession* self, DiplomatStringView channel_path, uint64_t timestamp_unix_ns, uint64_t sequence, DiplomatU8View value);

bool prismo_PluginSession_send_health(PluginSession* self, uint64_t emitted_updates, uint64_t dropped_updates, DiplomatStringView last_error);

bool prismo_PluginSession_send_log(PluginSession* self, DiplomatStringView level, DiplomatStringView message);


void prismo_PluginSession_destroy(PluginSession* self);





#endif // PluginSession_H
