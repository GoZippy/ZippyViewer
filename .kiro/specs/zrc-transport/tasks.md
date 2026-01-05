# Implementation Plan: zrc-transport

## Overview

Implementation tasks for the transport abstractions crate. This crate defines traits for different transport mechanisms and provides common framing, multiplexing, and testing utilities.

## Tasks

- [x] 1. Set up crate structure and dependencies
  - [x] 1.1 Create Cargo.toml with dependencies
    - tokio, async-trait, bytes, thiserror
    - _Requirements: 1.1, 1.8_
  - [x] 1.2 Create module structure
    - traits, framing, connection, backpressure, mux, metrics, testing
    - _Requirements: 1.1, 2.1, 4.1_

- [x] 2. Define transport traits
  - [x] 2.1 Define ControlPlaneTransport trait
    - send(recipient, envelope), recv() -> (sender, envelope)
    - is_connected(), transport_type()
    - _Requirements: 1.1, 1.4, 1.5_
  - [x] 2.2 Define MediaPlaneTransport trait
    - send_frame(channel, data), recv_frame()
    - congestion_state(), rtt_estimate()
    - _Requirements: 1.2, 1.6, 1.7_
  - [x] 2.3 Define DiscoveryTransport trait
    - publish(record), lookup(id), subscribe(id)
    - _Requirements: 1.3_
  - [x] 2.4 Define TransportError and common types
    - TransportType enum, SendResult, CongestionState
    - _Requirements: 1.8_

- [x] 3. Implement framing module
  - [x] 3.1 Implement LengthCodec
    - 4-byte big-endian length prefix
    - encode(data) and decode(framed)
    - _Requirements: 2.1, 2.2, 2.4, 2.5_
  - [x] 3.2 Implement streaming decoder
    - decode_stream(buf) for partial reads
    - _Requirements: 2.6_
  - [x] 3.3 Implement size limits and error handling
    - Max 64KB control, 1MB media
    - _Requirements: 2.3, 2.7_
  - [x]* 3.4 Write property test for framing round-trip
    - **Property 1: Framing Round-Trip**
    - **Validates: Requirements 2.8**

- [x] 4. Implement channel types
  - [x] 4.1 Define ChannelType enum
    - Control, Frames, Clipboard, Files, Audio
    - Priority and lossy flags
    - _Requirements: 7.1, 7.3_

- [x] 5. Implement connection state management
  - [x] 5.1 Define ConnectionState enum
    - Disconnected, Connecting, Connected, Reconnecting, Failed
    - _Requirements: 4.1_
  - [x] 5.2 Implement ConnectionManager
    - State transitions, duration tracking
    - Bytes sent/received counters
    - _Requirements: 4.2, 4.3, 4.4, 4.8_
  - [x] 5.3 Implement ConnectionStats
    - Aggregate statistics struct
    - _Requirements: 4.5_

- [x] 6. Implement backpressure handling
  - [x] 6.1 Define DropPolicy enum
    - Block, DropOldest, DropNewest, DropByPriority
    - _Requirements: 6.2, 6.3, 6.4_
  - [x] 6.2 Implement BackpressureHandler
    - can_send(size), reserve(size, channel), release(size)
    - Dropped frame tracking
    - _Requirements: 6.1, 6.5, 6.6, 6.7_
  - [x]* 6.3 Write property test for backpressure enforcement
    - **Property 3: Backpressure Enforcement**
    - **Validates: Requirements 6.2, 6.3**

- [x] 7. Checkpoint - Verify core transport abstractions
  - Ensure framing and backpressure tests pass
  - Ask the user if questions arise

- [x] 8. Implement transport ladder
  - [x] 8.1 Implement TransportLadder
    - Priority-ordered transport list
    - add(transport_type, transport)
    - _Requirements: 3.1, 3.2_
  - [x] 8.2 Implement connect (sequential)
    - Try transports in order, timeout handling
    - _Requirements: 3.3, 3.4, 3.8_
  - [x] 8.3 Implement connect_parallel
    - Parallel attempts, first success wins
    - _Requirements: 3.5, 3.6_
  - [x]* 8.4 Write property test for transport fallback
    - **Property 5: Transport Fallback**
    - **Validates: Requirements 3.4**

- [x] 9. Implement reconnection logic
  - [x] 9.1 Implement ReconnectionManager
    - Exponential backoff, max attempts
    - _Requirements: 5.1, 5.2, 5.3_
  - [x] 9.2 Implement session preservation
    - State preservation during reconnection
    - _Requirements: 5.4, 5.5_
  - [x] 9.3 Implement cancellation and failure reporting
    - _Requirements: 5.6, 5.7, 5.8_

- [x] 10. Implement multiplexer
  - [x] 10.1 Implement Multiplexer struct
    - Channel state management
    - open_channel, close_channel
    - _Requirements: 7.2, 7.5_
  - [x] 10.2 Implement send and recv
    - Route to correct channel
    - Per-channel sequence numbers
    - _Requirements: 7.4, 7.6_
  - [x] 10.3 Implement channel error isolation
    - Independent error handling
    - _Requirements: 7.7_
  - [x]* 10.4 Write property test for sequence monotonicity
    - **Property 2: Sequence Monotonicity**
    - **Validates: Requirements 5.4**
  - [x]* 10.5 Write property test for channel independence
    - **Property 4: Channel Independence**
    - **Validates: Requirements 7.7**

- [x] 11. Implement encryption integration
  - [x] 11.1 Define ChannelEncryption trait
    - encrypt(channel, seq, data), decrypt(channel, seq, data)
    - _Requirements: 8.1, 8.2, 8.3_
  - [x] 11.2 Implement encryption hooks in Multiplexer
    - set_encryption, key rotation support
    - _Requirements: 8.4, 8.7_
  - [x] 11.3 Implement key zeroization
    - _Requirements: 8.8_

- [x] 12. Implement metrics
  - [x] 12.1 Implement TransportMetrics
    - Counters for bytes, messages, drops
    - RTT histogram
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_
  - [x] 12.2 Implement Prometheus export
    - export_prometheus() method
    - _Requirements: 9.6, 9.7_

- [x] 13. Implement testing utilities
  - [x] 13.1 Implement MockTransport
    - Configurable latency and packet loss
    - inject_recv, get_sent, disconnect
    - _Requirements: 10.1, 10.3, 10.4, 10.5_
  - [x] 13.2 Implement LoopbackTransport
    - pair() creating connected pair
    - _Requirements: 10.2_
  - [x] 13.3 Implement transport recording
    - Record and replay for deterministic testing
    - _Requirements: 10.6, 10.7_

- [x] 14. Implement QUIC helpers
  - [x] 14.1 Implement QUIC stream mapping
    - Channel to stream ID mapping
    - _Requirements: 11.1_
  - [x] 14.2 Implement certificate utilities
    - Self-signed cert generation, pinning verification
    - _Requirements: 11.3, 11.4_
  - [x] 14.3 Implement QUIC configuration helpers
    - ALPN constants, congestion control config
    - _Requirements: 11.2, 11.5, 11.8_

- [x] 15. Implement HTTP mailbox helpers
  - [x] 15.1 Implement HTTP client configuration
    - Connection pooling, timeouts, proxy support
    - _Requirements: 12.1, 12.6, 12.7, 12.8_
  - [x] 15.2 Implement long-poll helpers
    - Retry logic with backoff
    - _Requirements: 12.2, 12.3_
  - [x] 15.3 Implement request/response utilities
    - Signing, parsing
    - _Requirements: 12.4, 12.5_

- [x] 16. Checkpoint - Verify all tests pass
  - Ensure all property tests pass with 100+ iterations
  - Verify transport abstractions work correctly
  - Ask the user if questions arise

## Notes

- Tasks marked with `*` are optional property-based tests
- This crate has no OS-specific dependencies
- Traits enable pluggable transport implementations
- Testing utilities are critical for integration testing
