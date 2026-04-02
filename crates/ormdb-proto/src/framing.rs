//! Message framing utilities for transport layer.
//!
//! This module provides length-prefix framing for messages sent over the wire.
//! The format is simple: 4-byte big-endian length prefix followed by the payload.

use crate::Error;

/// Maximum message size (4 MB) for security hardening.
/// Large payloads could be used for DoS attacks.
pub const MAX_MESSAGE_SIZE: usize = 4 * 1024 * 1024;

/// Size of the length prefix in bytes.
pub const LENGTH_PREFIX_SIZE: usize = 4;

/// Encode a payload with a length prefix.
///
/// Returns a new buffer containing `[length (4 bytes BE)][payload]`.
pub fn encode_frame(payload: &[u8]) -> Result<Vec<u8>, Error> {
    if payload.len() > MAX_MESSAGE_SIZE {
        return Err(Error::InvalidMessage(format!(
            "payload size {} exceeds maximum {}",
            payload.len(),
            MAX_MESSAGE_SIZE
        )));
    }

    let len = payload.len() as u32;
    let mut frame = Vec::with_capacity(LENGTH_PREFIX_SIZE + payload.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// Decode the length from a 4-byte header.
///
/// Returns the payload length as a usize.
pub fn decode_frame_length(header: &[u8; LENGTH_PREFIX_SIZE]) -> Result<usize, Error> {
    let len = u32::from_be_bytes(*header) as usize;
    if len > MAX_MESSAGE_SIZE {
        return Err(Error::InvalidMessage(format!(
            "frame length {} exceeds maximum {}",
            len, MAX_MESSAGE_SIZE
        )));
    }
    Ok(len)
}

/// Read a length prefix from a byte slice.
///
/// Returns the length and a slice starting after the prefix.
pub fn read_length_prefix(data: &[u8]) -> Result<(usize, &[u8]), Error> {
    if data.len() < LENGTH_PREFIX_SIZE {
        return Err(Error::InvalidMessage(format!(
            "buffer too short for length prefix: {} < {}",
            data.len(),
            LENGTH_PREFIX_SIZE
        )));
    }

    let mut header = [0u8; LENGTH_PREFIX_SIZE];
    header.copy_from_slice(&data[..LENGTH_PREFIX_SIZE]);
    let len = decode_frame_length(&header)?;

    Ok((len, &data[LENGTH_PREFIX_SIZE..]))
}

/// Validate that a buffer contains a complete frame.
///
/// Returns the total frame size (including prefix) if complete, or None if more data is needed.
pub fn frame_complete(data: &[u8]) -> Option<usize> {
    if data.len() < LENGTH_PREFIX_SIZE {
        return None;
    }

    let mut header = [0u8; LENGTH_PREFIX_SIZE];
    header.copy_from_slice(&data[..LENGTH_PREFIX_SIZE]);
    let len = u32::from_be_bytes(header) as usize;

    let total = LENGTH_PREFIX_SIZE + len;
    if data.len() >= total {
        Some(total)
    } else {
        None
    }
}

/// Extract the payload from a complete frame.
///
/// Assumes the frame is complete (use `frame_complete` to check first).
pub fn extract_payload(frame: &[u8]) -> Result<&[u8], Error> {
    if frame.len() < LENGTH_PREFIX_SIZE {
        return Err(Error::InvalidMessage("frame too short".to_string()));
    }

    let mut header = [0u8; LENGTH_PREFIX_SIZE];
    header.copy_from_slice(&frame[..LENGTH_PREFIX_SIZE]);
    let len = decode_frame_length(&header)?;

    if frame.len() < LENGTH_PREFIX_SIZE + len {
        return Err(Error::InvalidMessage(format!(
            "frame incomplete: have {}, need {}",
            frame.len(),
            LENGTH_PREFIX_SIZE + len
        )));
    }

    Ok(&frame[LENGTH_PREFIX_SIZE..LENGTH_PREFIX_SIZE + len])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_frame_empty() {
        let frame = encode_frame(&[]).unwrap();
        assert_eq!(frame.len(), LENGTH_PREFIX_SIZE);
        assert_eq!(&frame[..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn test_encode_frame_small() {
        let payload = b"hello";
        let frame = encode_frame(payload).unwrap();

        assert_eq!(frame.len(), LENGTH_PREFIX_SIZE + payload.len());
        // Length should be 5 in big-endian
        assert_eq!(&frame[..4], &[0, 0, 0, 5]);
        assert_eq!(&frame[4..], payload);
    }

    #[test]
    fn test_encode_frame_large() {
        let payload = vec![0u8; 1000];
        let frame = encode_frame(&payload).unwrap();

        assert_eq!(frame.len(), LENGTH_PREFIX_SIZE + 1000);
        // Length 1000 = 0x3E8 in big-endian
        assert_eq!(&frame[..4], &[0, 0, 0x03, 0xE8]);
    }

    #[test]
    fn test_encode_frame_too_large() {
        let payload = vec![0u8; MAX_MESSAGE_SIZE + 1];
        let result = encode_frame(&payload);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_frame_length() {
        // Zero length
        let header = [0, 0, 0, 0];
        assert_eq!(decode_frame_length(&header).unwrap(), 0);

        // Small length (5)
        let header = [0, 0, 0, 5];
        assert_eq!(decode_frame_length(&header).unwrap(), 5);

        // Larger length (1000)
        let header = [0, 0, 0x03, 0xE8];
        assert_eq!(decode_frame_length(&header).unwrap(), 1000);

        // Max valid length
        let max_len = MAX_MESSAGE_SIZE as u32;
        let header = max_len.to_be_bytes();
        assert_eq!(decode_frame_length(&header).unwrap(), MAX_MESSAGE_SIZE);
    }

    #[test]
    fn test_decode_frame_length_too_large() {
        let too_large = (MAX_MESSAGE_SIZE as u32) + 1;
        let header = too_large.to_be_bytes();
        let result = decode_frame_length(&header);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_length_prefix() {
        let data = [0, 0, 0, 5, 1, 2, 3, 4, 5, 6, 7];
        let (len, rest) = read_length_prefix(&data).unwrap();

        assert_eq!(len, 5);
        assert_eq!(rest, &[1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn test_read_length_prefix_too_short() {
        let data = [0, 0, 0];
        let result = read_length_prefix(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_frame_complete() {
        // Incomplete - not enough for header
        assert_eq!(frame_complete(&[0, 0, 0]), None);

        // Incomplete - header present but no payload
        assert_eq!(frame_complete(&[0, 0, 0, 5]), None);

        // Incomplete - partial payload
        assert_eq!(frame_complete(&[0, 0, 0, 5, 1, 2, 3]), None);

        // Complete - empty payload
        assert_eq!(frame_complete(&[0, 0, 0, 0]), Some(4));

        // Complete - with payload
        assert_eq!(frame_complete(&[0, 0, 0, 3, 1, 2, 3]), Some(7));

        // Complete - with extra data
        assert_eq!(frame_complete(&[0, 0, 0, 2, 1, 2, 3, 4, 5]), Some(6));
    }

    #[test]
    fn test_extract_payload() {
        // Empty payload
        let frame = [0, 0, 0, 0];
        let payload = extract_payload(&frame).unwrap();
        assert!(payload.is_empty());

        // Non-empty payload
        let frame = [0, 0, 0, 3, 1, 2, 3];
        let payload = extract_payload(&frame).unwrap();
        assert_eq!(payload, &[1, 2, 3]);

        // With extra data (should only extract declared length)
        let frame = [0, 0, 0, 2, 1, 2, 3, 4, 5];
        let payload = extract_payload(&frame).unwrap();
        assert_eq!(payload, &[1, 2]);
    }

    #[test]
    fn test_roundtrip() {
        let original = b"The quick brown fox jumps over the lazy dog";
        let frame = encode_frame(original).unwrap();
        let payload = extract_payload(&frame).unwrap();
        assert_eq!(payload, original);
    }

    #[test]
    fn test_roundtrip_binary() {
        let original: Vec<u8> = (0..=255).collect();
        let frame = encode_frame(&original).unwrap();
        let payload = extract_payload(&frame).unwrap();
        assert_eq!(payload, original.as_slice());
    }
}
