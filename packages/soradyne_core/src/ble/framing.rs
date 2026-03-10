//! Shared length-prefix wire framing for BLE and TCP transports.
//!
//! Frame format: `[u32 LE total_len][payload...]`
//!
//! For BLE transports the frame is then split into fixed-size chunks
//! (see `BLE_CHUNK_SIZE`). For TCP the entire frame is written at once.

/// Maximum bytes per GATT write or notification chunk.
/// Android caps GATT attribute values at 512 bytes, so we stay comfortably below.
pub const BLE_CHUNK_SIZE: usize = 500;

/// Build a length-prefixed frame: `[u32 LE len][data]`.
pub fn build_frame(data: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(4 + data.len());
    frame.extend_from_slice(&(data.len() as u32).to_le_bytes());
    frame.extend_from_slice(data);
    frame
}

/// Incremental reassembler for length-prefixed frames.
///
/// Feed chunks via `push()`, then call `try_extract()` to pull out
/// complete messages. Handles back-to-back frames and partial reads.
pub struct FrameReassembler {
    buf: Vec<u8>,
}

impl FrameReassembler {
    /// Create a new empty reassembler.
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Append raw bytes (one chunk, one TCP read, etc.).
    pub fn push(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Try to extract one complete message from the buffer.
    ///
    /// Returns `Some(payload)` if a full frame is available, `None` otherwise.
    /// Call in a loop to drain back-to-back frames.
    pub fn try_extract(&mut self) -> Option<Vec<u8>> {
        if self.buf.len() < 4 {
            return None;
        }
        let len = u32::from_le_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
        if self.buf.len() < 4 + len {
            return None;
        }
        let msg = self.buf[4..4 + len].to_vec();
        self.buf = self.buf[4 + len..].to_vec();
        Some(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_frame_roundtrip() {
        let data = b"hello world";
        let frame = build_frame(data);
        assert_eq!(frame.len(), 4 + data.len());
        let len = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        assert_eq!(len, data.len());
        assert_eq!(&frame[4..], data);
    }

    #[test]
    fn test_reassembler_single_message() {
        let mut r = FrameReassembler::new();
        let frame = build_frame(b"test");
        r.push(&frame);
        assert_eq!(r.try_extract(), Some(b"test".to_vec()));
        assert_eq!(r.try_extract(), None);
    }

    #[test]
    fn test_reassembler_chunked() {
        let mut r = FrameReassembler::new();
        let frame = build_frame(b"hello");
        // Feed one byte at a time
        for byte in &frame {
            r.push(&[*byte]);
        }
        assert_eq!(r.try_extract(), Some(b"hello".to_vec()));
    }

    #[test]
    fn test_reassembler_back_to_back() {
        let mut r = FrameReassembler::new();
        let mut combined = build_frame(b"first");
        combined.extend_from_slice(&build_frame(b"second"));
        r.push(&combined);
        assert_eq!(r.try_extract(), Some(b"first".to_vec()));
        assert_eq!(r.try_extract(), Some(b"second".to_vec()));
        assert_eq!(r.try_extract(), None);
    }

    #[test]
    fn test_reassembler_partial_header() {
        let mut r = FrameReassembler::new();
        r.push(&[0x05]); // Only 1 of 4 header bytes
        assert_eq!(r.try_extract(), None);
        r.push(&[0x00, 0x00, 0x00]); // Rest of header (len=5)
        assert_eq!(r.try_extract(), None); // No payload yet
        r.push(b"hel");
        assert_eq!(r.try_extract(), None); // Still partial
        r.push(b"lo");
        assert_eq!(r.try_extract(), Some(b"hello".to_vec()));
    }

    #[test]
    fn test_reassembler_empty_message() {
        let mut r = FrameReassembler::new();
        r.push(&build_frame(b""));
        assert_eq!(r.try_extract(), Some(b"".to_vec()));
    }

    #[test]
    fn test_ble_chunking() {
        // Verify that build_frame output can be split into BLE_CHUNK_SIZE chunks
        // and reassembled correctly.
        let data = vec![0xABu8; 2000]; // Larger than one BLE chunk
        let frame = build_frame(&data);
        let mut r = FrameReassembler::new();
        for chunk in frame.chunks(BLE_CHUNK_SIZE) {
            r.push(chunk);
        }
        assert_eq!(r.try_extract(), Some(data));
    }
}
