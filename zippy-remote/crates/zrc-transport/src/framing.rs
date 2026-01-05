//! Length-prefixed framing for reliable message delimiting.

use bytes::{Buf, BufMut, BytesMut};
use std::io;
use thiserror::Error;

/// Maximum frame size for control plane messages (64KB)
pub const MAX_CONTROL_FRAME_SIZE: usize = 64 * 1024;

/// Maximum frame size for media plane messages (1MB)
pub const MAX_MEDIA_FRAME_SIZE: usize = 1024 * 1024;

/// Framing error
#[derive(Debug, Error)]
pub enum FramingError {
    #[error("Frame too large: {0} bytes (max: {1})")]
    TooLarge(usize, usize),

    #[error("Incomplete frame: need {0} more bytes")]
    Incomplete(usize),

    #[error("Invalid frame format")]
    InvalidFormat,

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// Length-prefixed frame codec
pub struct LengthCodec {
    max_frame_size: usize,
}

impl LengthCodec {
    /// Create a new codec with the specified maximum frame size
    pub fn new(max_frame_size: usize) -> Self {
        Self { max_frame_size }
    }

    /// Create a codec for control plane messages
    pub fn control() -> Self {
        Self::new(MAX_CONTROL_FRAME_SIZE)
    }

    /// Create a codec for media plane messages
    pub fn media() -> Self {
        Self::new(MAX_MEDIA_FRAME_SIZE)
    }

    /// Encode data with length prefix
    /// Format: length (4 bytes BE) || data
    pub fn encode(&self, data: &[u8]) -> Result<Vec<u8>, FramingError> {
        if data.len() > self.max_frame_size {
            return Err(FramingError::TooLarge(data.len(), self.max_frame_size));
        }

        let mut encoded = Vec::with_capacity(4 + data.len());
        encoded.put_u32(data.len() as u32);
        encoded.extend_from_slice(data);
        Ok(encoded)
    }

    /// Decode framed data
    pub fn decode(&self, framed: &[u8]) -> Result<Vec<u8>, FramingError> {
        if framed.len() < 4 {
            return Err(FramingError::Incomplete(4 - framed.len()));
        }

        let mut buf = &framed[..];
        let len = buf.get_u32() as usize;

        if len > self.max_frame_size {
            return Err(FramingError::TooLarge(len, self.max_frame_size));
        }

        if buf.remaining() < len {
            return Err(FramingError::Incomplete(len - buf.remaining()));
        }

        Ok(buf[..len].to_vec())
    }

    /// Streaming decoder for partial reads
    /// Returns Some(data) when a complete frame is available, None if more data needed
    pub fn decode_stream(&self, buf: &mut BytesMut) -> Result<Option<Vec<u8>>, FramingError> {
        if buf.len() < 4 {
            return Ok(None);
        }

        let len = {
            let mut len_buf = &buf[..4];
            len_buf.get_u32() as usize
        };

        if len > self.max_frame_size {
            return Err(FramingError::TooLarge(len, self.max_frame_size));
        }

        if buf.len() < 4 + len {
            return Ok(None);
        }

        // Extract the frame
        buf.advance(4);
        let frame = buf.split_to(len).to_vec();
        Ok(Some(frame))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_encode_decode() {
        let codec = LengthCodec::control();
        let data = b"hello world";
        let encoded = codec.encode(data).unwrap();
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(data, decoded.as_slice());
    }

    #[test]
    fn test_too_large() {
        let codec = LengthCodec::control();
        let data = vec![0u8; MAX_CONTROL_FRAME_SIZE + 1];
        assert!(codec.encode(&data).is_err());
    }

    #[test]
    fn test_streaming_decoder() {
        let codec = LengthCodec::control();
        let data = b"hello";
        let encoded = codec.encode(data).unwrap();

        // Partial read
        let mut buf = BytesMut::from(&encoded[..2]);
        assert!(codec.decode_stream(&mut buf).unwrap().is_none());

        // Complete read
        buf.extend_from_slice(&encoded[2..]);
        let decoded = codec.decode_stream(&mut buf).unwrap().unwrap();
        assert_eq!(data, decoded.as_slice());
    }

    proptest! {
        #[test]
        fn prop_framing_round_trip(data in prop::collection::vec(any::<u8>(), 0..MAX_CONTROL_FRAME_SIZE)) {
            let codec = LengthCodec::control();
            let encoded = codec.encode(&data)?;
            let decoded = codec.decode(&encoded)?;
            prop_assert_eq!(data, decoded);
        }
    }
}
