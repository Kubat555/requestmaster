//! Streaming of a response body to disk with decoding by `Content-Encoding`.
//!
//! Runs on a blocking thread: the engine feeds received chunks through a bounded channel, so memory does not
//! grow with the body size. While a body is being decoded, a copy as received is kept in a second file; it
//! replaces the decoded file when decoding fails and is removed otherwise.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use bytes::Bytes;
use tokio::sync::mpsc::Receiver;

/// A message from the engine to the body writer.
pub(crate) enum Piece {
    /// The next chunk of the body as received.
    Data(Bytes),
    /// The body was received completely. A channel closed without it means the send was aborted.
    End,
}

/// A content coding the engine can undo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Coding {
    Gzip,
    Deflate,
    Brotli,
    Zstd,
}

/// Result of storing a complete body.
pub(crate) struct Written {
    /// Body bytes as received.
    pub wire_size: u64,
    /// Size of the stored file.
    pub decoded_size: u64,
    /// Why decoding failed; the stored file is then the body as received.
    pub decode_error: Option<String>,
}

/// Codings of a response in the order they were applied, from its `Content-Encoding` header values.
/// Empty when there is nothing to undo: no header, only `identity`, or a coding the engine does not know
/// (the body is then stored as received).
pub(crate) fn codings<'a>(values: impl Iterator<Item = &'a str>) -> Vec<Coding> {
    let mut codings = Vec::new();
    for token in values.flat_map(|value| value.split(',')) {
        match token.trim().to_ascii_lowercase().as_str() {
            "" | "identity" => {}
            "gzip" | "x-gzip" => codings.push(Coding::Gzip),
            "deflate" => codings.push(Coding::Deflate),
            "br" => codings.push(Coding::Brotli),
            "zstd" => codings.push(Coding::Zstd),
            _ => return Vec::new(),
        }
    }
    codings
}

/// Receives the body from `pieces` and stores it decoded at `body_path`.
///
/// `Ok(None)` — the send was aborted (the channel closed without [`Piece::End`]). In that case and on a file
/// error nothing is left on disk.
pub(crate) fn write_body(pieces: Receiver<Piece>, body_path: &Path, codings: &[Coding]) -> io::Result<Option<Written>> {
    let wire_path = body_path.with_extension("wire");
    let result = store(pieces, body_path, &wire_path, codings);
    if !matches!(result, Ok(Some(_))) {
        // Either file may not exist yet; a leftover cannot be reported any better than the original failure.
        let _ = std::fs::remove_file(body_path);
        let _ = std::fs::remove_file(&wire_path);
    }
    result
}

fn store(pieces: Receiver<Piece>, body_path: &Path, wire_path: &Path, codings: &[Coding]) -> io::Result<Option<Written>> {
    if codings.is_empty() {
        let mut wire = Wire::new(pieces, File::create(body_path)?);
        wire.drain()?;
        return Ok(wire.complete.then_some(Written { wire_size: wire.size, decoded_size: wire.size, decode_error: None }));
    }

    let mut wire = Wire::new(pieces, File::create(wire_path)?);
    let mut decoded_file = File::create(body_path)?;
    // An empty body is not decoded: `HEAD`, `204` and `304` carry `Content-Encoding` without a body.
    let decoded = if wire.fill() { decode(&mut wire, codings, &mut decoded_file)? } else { Ok(0) };
    if let Some(error) = wire.copy_error.take() {
        return Err(error);
    }
    // After a decoding failure (or trailing bytes the decoder did not ask for) the rest still goes to the copy.
    wire.drain()?;
    if !wire.complete {
        return Ok(None);
    }
    let wire_size = wire.size;
    // Both files must be closed before one of them is removed or replaced.
    drop(wire);
    drop(decoded_file);
    Ok(Some(match decoded {
        Ok(decoded_size) => {
            std::fs::remove_file(wire_path)?;
            Written { wire_size, decoded_size, decode_error: None }
        }
        Err(decode_error) => {
            std::fs::rename(wire_path, body_path)?;
            Written { wire_size, decoded_size: wire_size, decode_error: Some(decode_error) }
        }
    }))
}

/// Decodes `wire` into `out`. The outer result is a failure to write `out`; the inner one is the decoded
/// size or the decoder's complaint about the data.
fn decode(wire: &mut Wire, codings: &[Coding], out: &mut File) -> io::Result<Result<u64, String>> {
    let mut reader = match decoder(wire, codings) {
        Ok(reader) => reader,
        Err(error) => return Ok(Err(error.to_string())),
    };
    let mut buffer = vec![0u8; 64 * 1024];
    let mut decoded_size = 0u64;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(Ok(decoded_size)),
            Ok(n) => {
                out.write_all(&buffer[..n])?;
                decoded_size += n as u64;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Ok(Err(error.to_string())),
        }
    }
}

/// Chain of decoders over `wire`: the coding applied last is undone first.
fn decoder<'a>(wire: &'a mut Wire, codings: &[Coding]) -> io::Result<Box<dyn Read + 'a>> {
    let mut reader: Box<dyn Read + 'a> = Box::new(wire);
    for coding in codings.iter().rev() {
        reader = match coding {
            Coding::Gzip => Box::new(flate2::read::MultiGzDecoder::new(reader)),
            Coding::Deflate => deflate_decoder(reader)?,
            Coding::Brotli => Box::new(brotli::Decompressor::new(reader, 64 * 1024)),
            Coding::Zstd => Box::new(zstd::stream::read::Decoder::new(reader)?),
        };
    }
    Ok(reader)
}

/// HTTP `deflate` is meant to be a zlib stream, but some servers send raw deflate: the first two bytes tell.
fn deflate_decoder<'a>(mut reader: Box<dyn Read + 'a>) -> io::Result<Box<dyn Read + 'a>> {
    let mut head = [0u8; 2];
    let mut filled = 0;
    while filled < head.len() {
        match reader.read(&mut head[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    // zlib header: compression method 8 in the low nibble, and the two bytes form a multiple of 31.
    let is_zlib = filled == 2 && head[0] & 0x0f == 8 && u16::from_be_bytes(head) % 31 == 0;
    let reader = io::Cursor::new(head).take(filled as u64).chain(reader);
    Ok(if is_zlib { Box::new(flate2::read::ZlibDecoder::new(reader)) } else { Box::new(flate2::read::DeflateDecoder::new(reader)) })
}

/// The body as received, read from the channel; everything read is also written to `copy` and counted.
struct Wire {
    pieces: Receiver<Piece>,
    copy: File,
    /// Received bytes not handed out yet.
    current: Bytes,
    /// Bytes handed out so far.
    size: u64,
    /// The channel has no more data.
    ended: bool,
    /// [`Piece::End`] was received.
    complete: bool,
    /// A failure to write `copy` that was reported to a decoder (which turns it into its own error).
    copy_error: Option<io::Error>,
}

impl Wire {
    fn new(pieces: Receiver<Piece>, copy: File) -> Self {
        Wire { pieces, copy, current: Bytes::new(), size: 0, ended: false, complete: false, copy_error: None }
    }

    /// Makes `current` non-empty; `false` when the body has ended.
    fn fill(&mut self) -> bool {
        while self.current.is_empty() {
            if self.ended {
                return false;
            }
            match self.pieces.blocking_recv() {
                Some(Piece::Data(data)) => self.current = data,
                Some(Piece::End) => {
                    self.complete = true;
                    self.ended = true;
                }
                None => self.ended = true,
            }
        }
        true
    }

    /// Writes the rest of the body to `copy`.
    fn drain(&mut self) -> io::Result<()> {
        while self.fill() {
            let data = std::mem::take(&mut self.current);
            self.copy.write_all(&data)?;
            self.size += data.len() as u64;
        }
        Ok(())
    }
}

impl Read for Wire {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(error) = &self.copy_error {
            return Err(io::Error::new(error.kind(), error.to_string()));
        }
        if buf.is_empty() || !self.fill() {
            return Ok(0);
        }
        let data = self.current.split_to(buf.len().min(self.current.len()));
        if let Err(error) = self.copy.write_all(&data) {
            self.copy_error = Some(io::Error::new(error.kind(), error.to_string()));
            return Err(error);
        }
        buf[..data.len()].copy_from_slice(&data);
        self.size += data.len() as u64;
        Ok(data.len())
    }
}
