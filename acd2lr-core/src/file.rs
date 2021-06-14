use std::{
    fs::File,
    io::BufReader,
    io::{prelude::*, SeekFrom},
    ops::Range,
};

use thiserror::Error;

#[derive(Debug)]
pub struct XPacketFile {
    fh: File,
    span: Option<Range<usize>>,
}

impl XPacketFile {
    fn no_xpacket(buf: BufReader<File>) -> Self {
        Self {
            fh: buf.into_inner(),
            span: None,
        }
    }

    fn with_xpacket(buf: BufReader<File>, range: Range<usize>) -> Self {
        Self {
            fh: buf.into_inner(),
            span: Some(range),
        }
    }

    fn find_needle(
        buf: &mut BufReader<File>,
        needle: &[u8],
        buffer: &mut [u8],
    ) -> std::io::Result<Option<usize>> {
        // Look for the packet beginning
        loop {
            if let Ok(_) = buf.read_exact(buffer) {
                // read enough bytes

                if let Some(idx) = memchr::memchr(needle[0], &buffer) {
                    // Start char found in the buffer

                    let left_in_haystack = buffer.len() - idx;
                    if left_in_haystack >= needle.len() {
                        // The needle may be at idx

                        if &buffer[idx..(idx + needle.len())] == needle {
                            // We found the needle at idx
                            let needle_idx = buf.stream_position()? as usize - left_in_haystack;
                            // Seek back
                            buf.seek(SeekFrom::Start(needle_idx as _))?;
                            return Ok(Some(needle_idx));
                        } else {
                            // We didn't find the needle at idx, seek back and repeat read
                            buf.seek(SeekFrom::Current(-((left_in_haystack - 1) as i64)))?;
                        }
                    } else {
                        // There's not enough left for the needle
                        buf.seek(SeekFrom::Current(-(left_in_haystack as i64)))?;
                    }
                } else {
                    // Start char not found in the entire buffer, so we can skip away
                }
            } else {
                // eof
                return Ok(None);
            }
        }
    }

    pub fn file(&self) -> &File {
        &self.fh
    }

    pub fn open(file: File) -> std::io::Result<Self> {
        // Wrap with a BufReader
        let mut buf = BufReader::new(file);

        // Buffer for looking for markers
        let mut haystack_buffer: [u8; 128] = [0; 128];

        // Find xpacket beginning
        const XPACKET_BEGIN: &[u8] = b"<?xpacket begin";
        let start = if let Some(start) = Self::find_needle(
            &mut buf,
            &XPACKET_BEGIN,
            &mut haystack_buffer[..XPACKET_BEGIN.len()],
        )? {
            start
        } else {
            return Ok(Self::no_xpacket(buf));
        };

        // Find xpacket end, starting at the current position
        const XPACKET_END: &[u8] = b"<?xpacket end";
        let _ = if let Some(_) = Self::find_needle(
            &mut buf,
            &XPACKET_END,
            &mut haystack_buffer[..XPACKET_END.len()],
        )? {
            // nothing to do, we use this to advance the stream
        } else {
            return Ok(Self::no_xpacket(buf));
        };

        // After the start of the end marker, we want to find the ?> that marks the actual end
        const BOUND_MARKER: &[u8] = b"?>";
        let end = if let Some(end) = Self::find_needle(
            &mut buf,
            &BOUND_MARKER,
            &mut haystack_buffer[..BOUND_MARKER.len()],
        )? {
            // We want the end of the needle to return [start, end)
            end + BOUND_MARKER.len()
        } else {
            return Ok(Self::no_xpacket(buf));
        };

        Ok(Self::with_xpacket(buf, start..end))
    }

    pub fn read_packet_bytes(&mut self) -> std::io::Result<Option<Vec<u8>>> {
        if let Some(range) = self.span.clone() {
            self.fh.seek(SeekFrom::Start(range.start as _))?;

            let mut buf = vec![0; range.len()];
            self.fh.read_exact(&mut buf[..])?;

            Ok(Some(buf))
        } else {
            Ok(None)
        }
    }

    pub fn write_packet_bytes(&mut self, new_bytes: &[u8]) -> Result<(), WritePacketError> {
        if let Some(range) = self.span.clone() {
            if range.len() != new_bytes.len() {
                return Err(WritePacketError::WrongPacketSize);
            }

            // Seek to the beginning of the packet
            self.fh.seek(SeekFrom::Start(range.start as _))?;

            // Write the packet
            self.fh.write_all(new_bytes)?;

            Ok(())
        } else {
            Err(WritePacketError::NoPacket)
        }
    }
}

#[derive(Debug, Error)]
pub enum WritePacketError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("no packet in this file")]
    NoPacket,
    #[error("packet size does not match physical packet size")]
    WrongPacketSize,
}
