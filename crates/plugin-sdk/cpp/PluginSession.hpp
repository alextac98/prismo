#pragma once

#include <cstdint>
#include <cstdio>
#include <cstddef>
#include <cstring>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <vector>

class PluginSession
{
  public:
    class Result
    {
      public:
        explicit Result( bool ok )
            : ok_( ok )
        {}

        std::optional<bool> ok() const { return ok_; }

      private:
        bool ok_;
    };

    enum class ValueType : std::uint32_t
    {
        Unspecified = 0,
        Bool        = 1,
        Integer     = 2,
        Float       = 3,
        Text        = 4,
        Bytes       = 5,
        Enum        = 6,
        Array       = 7,
    };

    class Value
    {
      public:
        Value( const Value& )            = default;
        Value( Value&& )                 = default;
        Value& operator=( const Value& ) = default;
        Value& operator=( Value&& )      = default;

      private:
        Value( ValueType leaf_type, std::uint32_t dimensions, std::vector<std::uint8_t> encoded )
            : leaf_type_( leaf_type )
            , dimensions_( dimensions )
            , encoded_( std::move( encoded ) )
        {}

        ValueType leaf_type_;
        std::uint32_t dimensions_;
        std::vector<std::uint8_t> encoded_;

        friend class PluginSession;
    };

    static Value bool_value( bool value )
    {
        std::vector<std::uint8_t> encoded;
        put_bool( encoded, static_cast<std::uint32_t>( ValueType::Bool ), value );
        return Value( ValueType::Bool, 0, std::move( encoded ) );
    }

    static Value integer_value( std::int64_t value )
    {
        std::vector<std::uint8_t> encoded;
        put_i64( encoded, static_cast<std::uint32_t>( ValueType::Integer ), value );
        return Value( ValueType::Integer, 0, std::move( encoded ) );
    }

    static Value float_value( double value )
    {
        std::vector<std::uint8_t> encoded;
        put_f64( encoded, static_cast<std::uint32_t>( ValueType::Float ), value );
        return Value( ValueType::Float, 0, std::move( encoded ) );
    }

    static Value text_value( const char* value )
    {
        std::vector<std::uint8_t> encoded;
        put_string( encoded, static_cast<std::uint32_t>( ValueType::Text ), value );
        return Value( ValueType::Text, 0, std::move( encoded ) );
    }

    static std::optional<Value> bytes_value( const std::uint8_t* value, std::size_t size )
    {
        if ( value == nullptr && size > 0 ) {
            return std::nullopt;
        }

        std::vector<std::uint8_t> encoded;
        put_key( encoded, static_cast<std::uint32_t>( ValueType::Bytes ), 2 );
        put_varint( encoded, size );
        if ( size > 0 ) {
            encoded.insert( encoded.end(), value, value + size );
        }
        return Value( ValueType::Bytes, 0, std::move( encoded ) );
    }

    static Value enum_value( std::int64_t value, const char* name )
    {
        std::vector<std::uint8_t> enum_value;
        put_i64( enum_value, 1, value );
        put_string( enum_value, 2, name );

        std::vector<std::uint8_t> encoded;
        put_message( encoded, static_cast<std::uint32_t>( ValueType::Enum ), enum_value );
        return Value( ValueType::Enum, 0, std::move( encoded ) );
    }

    static std::optional<Value> array_value( ValueType leaf_type,
                                             std::uint32_t dimensions,
                                             std::vector<Value> values )
    {
        if ( !is_scalar_value_type( leaf_type ) || dimensions == 0 ) {
            return std::nullopt;
        }

        for ( const auto& value : values ) {
            const bool valid = dimensions == 1
                                   ? value.dimensions_ == 0 && value.leaf_type_ == leaf_type
                                   : value.dimensions_ == dimensions - 1 && value.leaf_type_ == leaf_type;
            if ( !valid ) {
                return std::nullopt;
            }
        }

        std::vector<std::uint8_t> array;
        put_u64( array, 1, static_cast<std::uint32_t>( leaf_type ) );
        put_u64( array, 2, dimensions );
        for ( const auto& value : values ) {
            put_message( array, 3, value.encoded_ );
        }

        std::vector<std::uint8_t> encoded;
        put_message( encoded, static_cast<std::uint32_t>( ValueType::Array ), array );
        return Value( leaf_type, dimensions, std::move( encoded ) );
    }

    static std::unique_ptr<PluginSession> from_stdio()
    {
        InitConfig init;
        if ( !read_init( init ) ) {
            return nullptr;
        }
        return std::unique_ptr<PluginSession>( new PluginSession( std::move( init ) ) );
    }

    const std::string& plugin_id() const { return init_.plugin_id; }
    const std::string& config_json() const { return init_.config_json; }

    Result send_value_sample( const char* channel_path,
                              std::uint64_t timestamp_unix_ns,
                              std::uint64_t sequence,
                              const Value& value )
    {
        return send_sample( channel_path, timestamp_unix_ns, sequence, value.encoded_ );
    }

    Result send_hello( const char* plugin_version, const char* language )
    {
        std::vector<std::uint8_t> message;
        put_u64( message, 1, protocol_version );
        put_string( message, 2, init_.plugin_id );
        put_string( message, 3, plugin_version );
        put_string( message, 4, language );
        return Result( write_envelope( EnvelopeTag::Hello, message ) );
    }

    Result declare_channel( const char* channel_path,
                            const char* display_name,
                            const char* unit,
                            const char* description )
    {
        std::vector<std::uint8_t> channel;
        put_string( channel, 1, channel_path );
        put_string( channel, 2, display_name );
        if ( unit != nullptr && unit[ 0 ] != '\0' ) {
            put_string( channel, 3, unit );
        }
        put_string( channel, 4, description );

        std::vector<std::uint8_t> message;
        put_string( message, 1, init_.plugin_id );
        put_message( message, 2, channel );
        return Result( write_envelope( EnvelopeTag::DeclareChannels, message ) );
    }

    Result send_bool_sample( const char* channel_path,
                             std::uint64_t timestamp_unix_ns,
                             std::uint64_t sequence,
                             bool value )
    {
        return send_value_sample( channel_path, timestamp_unix_ns, sequence, bool_value( value ) );
    }

    Result send_integer_sample( const char* channel_path,
                                std::uint64_t timestamp_unix_ns,
                                std::uint64_t sequence,
                                std::int64_t value )
    {
        return send_value_sample( channel_path, timestamp_unix_ns, sequence, integer_value( value ) );
    }

    Result send_float_sample( const char* channel_path,
                              std::uint64_t timestamp_unix_ns,
                              std::uint64_t sequence,
                              double value )
    {
        return send_value_sample( channel_path, timestamp_unix_ns, sequence, float_value( value ) );
    }

    Result send_text_sample( const char* channel_path,
                             std::uint64_t timestamp_unix_ns,
                             std::uint64_t sequence,
                             const char* value )
    {
        return send_value_sample( channel_path, timestamp_unix_ns, sequence, text_value( value ) );
    }

    Result send_bytes_sample( const char* channel_path,
                              std::uint64_t timestamp_unix_ns,
                              std::uint64_t sequence,
                              const std::uint8_t* value,
                              std::size_t size )
    {
        const auto encoded = bytes_value( value, size );
        if ( !encoded ) {
            return Result( false );
        }
        return send_value_sample( channel_path, timestamp_unix_ns, sequence, *encoded );
    }

    Result send_enum_sample( const char* channel_path,
                             std::uint64_t timestamp_unix_ns,
                             std::uint64_t sequence,
                             std::int64_t value,
                             const char* name )
    {
        return send_value_sample( channel_path, timestamp_unix_ns, sequence, enum_value( value, name ) );
    }

    Result send_health( std::uint64_t emitted_updates, std::uint64_t dropped_updates, const char* last_error )
    {
        std::vector<std::uint8_t> message;
        put_string( message, 1, init_.plugin_id );
        put_u64( message, 2, emitted_updates );
        put_u64( message, 3, dropped_updates );
        if ( last_error != nullptr && last_error[ 0 ] != '\0' ) {
            put_string( message, 4, last_error );
        }
        return Result( write_envelope( EnvelopeTag::Health, message ) );
    }

    Result send_log( const char* level, const char* message_text )
    {
        std::vector<std::uint8_t> message;
        put_string( message, 1, init_.plugin_id );
        put_string( message, 2, level );
        put_string( message, 3, message_text );
        return Result( write_envelope( EnvelopeTag::Log, message ) );
    }

  private:
    enum class EnvelopeTag : std::uint32_t
    {
        Init            = 1,
        Hello           = 2,
        DeclareChannels = 3,
        SampleBatch     = 4,
        Health          = 5,
        Log             = 7,
    };

    struct InitConfig {
        std::uint32_t protocol_version = 0;
        std::string plugin_id = "unknown";
        std::string config_json = "{}";
    };

    static constexpr std::uint32_t protocol_version = 1;
    static constexpr std::uint32_t max_frame_bytes  = 8U * 1024U * 1024U;

    static bool is_scalar_value_type( ValueType value )
    {
        return value >= ValueType::Bool && value <= ValueType::Enum;
    }

    explicit PluginSession( InitConfig init )
        : init_( std::move( init ) )
    {}

    Result send_sample( const char* channel_path,
                        std::uint64_t timestamp_unix_ns,
                        std::uint64_t sequence,
                        const std::vector<std::uint8_t>& encoded_value )
    {
        std::vector<std::uint8_t> sample;
        put_string( sample, 1, channel_path );
        put_u64( sample, 2, timestamp_unix_ns );
        put_u64( sample, 3, sequence );
        put_message( sample, 4, encoded_value );

        std::vector<std::uint8_t> message;
        put_string( message, 1, init_.plugin_id );
        put_message( message, 2, sample );
        return Result( write_envelope( EnvelopeTag::SampleBatch, message ) );
    }

    static bool read_exact( std::uint8_t* data, std::size_t size ) { return std::fread( data, 1, size, stdin ) == size; }

    static bool read_varint( const std::vector<std::uint8_t>& in, std::size_t& offset, std::uint64_t& value )
    {
        value = 0;
        for ( std::uint32_t shift = 0; shift < 64U && offset < in.size(); shift += 7U ) {
            const std::uint8_t byte = in[ offset++ ];
            value |= static_cast<std::uint64_t>( byte & 0x7fU ) << shift;
            if ( ( byte & 0x80U ) == 0 ) {
                return true;
            }
        }
        return false;
    }

    static bool skip_field( const std::vector<std::uint8_t>& in, std::size_t& offset, std::uint32_t wire_type )
    {
        std::uint64_t ignored = 0;
        if ( wire_type == 0 ) {
            return read_varint( in, offset, ignored );
        }
        if ( wire_type == 1 && offset + 8 <= in.size() ) {
            offset += 8;
            return true;
        }
        if ( wire_type == 2 && read_varint( in, offset, ignored ) && offset + ignored <= in.size() ) {
            offset += static_cast<std::size_t>( ignored );
            return true;
        }
        if ( wire_type == 5 && offset + 4 <= in.size() ) {
            offset += 4;
            return true;
        }
        return false;
    }

    static std::string parse_string_field( const std::vector<std::uint8_t>& in, std::size_t& offset )
    {
        std::uint64_t size = 0;
        if ( !read_varint( in, offset, size ) || offset + size > in.size() ) {
            return {};
        }

        std::string value( reinterpret_cast<const char*>( in.data() + offset ), static_cast<std::size_t>( size ) );
        offset += static_cast<std::size_t>( size );
        return value;
    }

    static bool read_init( InitConfig& config )
    {
        std::uint8_t len_bytes[ 4 ] = {};
        if ( !read_exact( len_bytes, sizeof( len_bytes ) ) ) {
            return false;
        }

        const std::uint32_t frame_size = static_cast<std::uint32_t>( len_bytes[ 0 ] )
                                         | ( static_cast<std::uint32_t>( len_bytes[ 1 ] ) << 8U )
                                         | ( static_cast<std::uint32_t>( len_bytes[ 2 ] ) << 16U )
                                         | ( static_cast<std::uint32_t>( len_bytes[ 3 ] ) << 24U );
        if ( frame_size > max_frame_bytes ) {
            return false;
        }

        std::vector<std::uint8_t> frame( frame_size );
        if ( !read_exact( frame.data(), frame.size() ) ) {
            return false;
        }

        std::size_t offset = 0;
        while ( offset < frame.size() ) {
            std::uint64_t key = 0;
            if ( !read_varint( frame, offset, key ) ) {
                return false;
            }

            const auto field     = static_cast<std::uint32_t>( key >> 3U );
            const auto wire_type = static_cast<std::uint32_t>( key & 0x7U );
            if ( field != static_cast<std::uint32_t>( EnvelopeTag::Init ) || wire_type != 2 ) {
                if ( !skip_field( frame, offset, wire_type ) ) {
                    return false;
                }
                continue;
            }

            std::uint64_t init_size = 0;
            if ( !read_varint( frame, offset, init_size ) || offset + init_size > frame.size() ) {
                return false;
            }

            const std::vector<std::uint8_t> init( frame.begin() + static_cast<std::ptrdiff_t>( offset ),
                                                  frame.begin() + static_cast<std::ptrdiff_t>( offset + init_size ) );
            std::size_t init_offset = 0;
            while ( init_offset < init.size() ) {
                std::uint64_t init_key = 0;
                if ( !read_varint( init, init_offset, init_key ) ) {
                    return false;
                }

                const auto init_field     = static_cast<std::uint32_t>( init_key >> 3U );
                const auto init_wire_type = static_cast<std::uint32_t>( init_key & 0x7U );
                if ( init_field == 1 && init_wire_type == 0 ) {
                    std::uint64_t version = 0;
                    if ( !read_varint( init, init_offset, version ) ) {
                        return false;
                    }
                    config.protocol_version = static_cast<std::uint32_t>( version );
                } else if ( init_field == 3 && init_wire_type == 2 ) {
                    config.plugin_id = parse_string_field( init, init_offset );
                } else if ( init_field == 4 && init_wire_type == 2 ) {
                    config.config_json = parse_string_field( init, init_offset );
                } else if ( !skip_field( init, init_offset, init_wire_type ) ) {
                    return false;
                }
            }
            return config.protocol_version == protocol_version;
        }

        return false;
    }

    static void put_varint( std::vector<std::uint8_t>& out, std::uint64_t value )
    {
        while ( value >= 0x80U ) {
            out.push_back( static_cast<std::uint8_t>( value | 0x80U ) );
            value >>= 7U;
        }
        out.push_back( static_cast<std::uint8_t>( value ) );
    }

    static void put_key( std::vector<std::uint8_t>& out, std::uint32_t field, std::uint32_t wire_type )
    {
        put_varint( out, ( static_cast<std::uint64_t>( field ) << 3U ) | wire_type );
    }

    static void put_u64( std::vector<std::uint8_t>& out, std::uint32_t field, std::uint64_t value )
    {
        put_key( out, field, 0 );
        put_varint( out, value );
    }

    static void put_i64( std::vector<std::uint8_t>& out, std::uint32_t field, std::int64_t value )
    {
        put_u64( out, field, static_cast<std::uint64_t>( value ) );
    }

    static void put_bool( std::vector<std::uint8_t>& out, std::uint32_t field, bool value )
    {
        put_u64( out, field, value ? 1U : 0U );
    }

    static void put_f64( std::vector<std::uint8_t>& out, std::uint32_t field, double value )
    {
        put_key( out, field, 1 );
        std::uint64_t bits = 0;
        std::memcpy( &bits, &value, sizeof( bits ) );
        for ( std::size_t i = 0; i < sizeof( bits ); ++i ) {
            out.push_back( static_cast<std::uint8_t>( ( bits >> ( 8U * i ) ) & 0xffU ) );
        }
    }

    static void put_string( std::vector<std::uint8_t>& out, std::uint32_t field, const std::string& value )
    {
        put_key( out, field, 2 );
        put_varint( out, value.size() );
        out.insert( out.end(), value.begin(), value.end() );
    }

    static void put_string( std::vector<std::uint8_t>& out, std::uint32_t field, const char* value )
    {
        put_string( out, field, std::string( value == nullptr ? "" : value ) );
    }

    static void put_message( std::vector<std::uint8_t>& out,
                             std::uint32_t field,
                             const std::vector<std::uint8_t>& value )
    {
        put_key( out, field, 2 );
        put_varint( out, value.size() );
        out.insert( out.end(), value.begin(), value.end() );
    }

    static bool write_envelope( EnvelopeTag tag, const std::vector<std::uint8_t>& message )
    {
        std::vector<std::uint8_t> envelope;
        put_message( envelope, static_cast<std::uint32_t>( tag ), message );
        const auto size = static_cast<std::uint32_t>( envelope.size() );
        const std::uint8_t len_bytes[ 4 ] = {
            static_cast<std::uint8_t>( size & 0xffU ),
            static_cast<std::uint8_t>( ( size >> 8U ) & 0xffU ),
            static_cast<std::uint8_t>( ( size >> 16U ) & 0xffU ),
            static_cast<std::uint8_t>( ( size >> 24U ) & 0xffU ),
        };
        return std::fwrite( len_bytes, 1, sizeof( len_bytes ), stdout ) == sizeof( len_bytes )
               && std::fwrite( envelope.data(), 1, envelope.size(), stdout ) == envelope.size()
               && std::fflush( stdout ) == 0;
    }

    InitConfig init_;
};
