// SPDX-FileCopyrightText: 2026 Manuel Quarneti <mq1@ik.me>
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(clippy::all, rust_2018_idioms)]

use futures_lite::{AsyncWrite, AsyncWriteExt};
use thiserror::Error;

pub const WIILOAD_PORT: u16 = 4299;
const WIILOAD_MAGIC: [u8; 4] = *b"HAXX";
const WIILOAD_VERSION_MAJOR: u8 = 0;
const WIILOAD_VERSION_MINOR: u8 = 5;
const CHUNK_SIZE: usize = 1024 * 128;

#[derive(Error, Debug)]
pub enum WiiloadError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Network error: {0}")]
    Net(#[from] std::net::AddrParseError),
    #[error("Timeout")]
    Timeout,
    #[error("File > 4 GiB")]
    FileTooBig,
    #[error("Filename > 255 bytes")]
    FileNameTooLong,
}

#[repr(C)]
struct Header {
    magic: [u8; 4],
    version_major: u8,
    version_minor: u8,
    filename_len: u16,
    compressed_size: u32,
    uncompressed_size: u32,
}

impl Header {
    fn new(filename_len: u16, compressed_size: u32, uncompressed_size: u32) -> Self {
        Self {
            magic: WIILOAD_MAGIC,
            version_major: WIILOAD_VERSION_MAJOR,
            version_minor: WIILOAD_VERSION_MINOR,
            filename_len: filename_len.to_be(),
            compressed_size: compressed_size.to_be(),
            uncompressed_size: uncompressed_size.to_be(),
        }
    }

    fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                &self as *const _ as *const u8,
                std::mem::size_of::<Header>(),
            )
        }
    }
}

async fn push<W: AsyncWrite + Unpin>(
    writer: &mut W,
    filename: &str,
    body: &[u8],
    uncompressed_size: u32,
) -> Result<(), WiiloadError> {
    let compressed_size: u32 = body
        .len()
        .try_into()
        .map_err(|_| WiiloadError::FileTooBig)?;

    if filename.len() > 255 {
        return Err(WiiloadError::FileNameTooLong);
    }

    // Send Wiiload header
    let header = Header::new(filename.len() as u16, compressed_size, uncompressed_size);
    writer.write_all(header.as_bytes()).await?;

    // Send the data
    for chunk in body.chunks(CHUNK_SIZE) {
        writer.write_all(chunk).await?;
    }

    // Send filename with null terminator
    let mut buf = [0u8; 256];
    buf[..filename.len()].copy_from_slice(filename.as_bytes());
    writer.write_all(&buf[..filename.len() + 1]).await?;

    Ok(())
}

/// Sends a file to the Wii without applying any compression.
pub async fn send<W: AsyncWrite + Unpin>(
    writer: &mut W,
    filename: impl AsRef<str>,
    body: impl AsRef<[u8]>,
) -> Result<(), WiiloadError> {
    push(writer, filename.as_ref(), body.as_ref(), 0).await
}

/// Compresses the file data using Zlib and then sends it to the Wii.
/// Uses deflate -9 to minimize network transfer time.
#[cfg(feature = "compression")]
pub async fn compress_then_send<W: AsyncWrite + Unpin>(
    writer: &mut W,
    filename: impl AsRef<str>,
    body: impl AsRef<[u8]>,
) -> Result<(), WiiloadError> {
    use miniz_oxide::deflate::compress_to_vec_zlib;

    let body = body.as_ref();
    let uncompressed_size = body
        .len()
        .try_into()
        .map_err(|_| WiiloadError::FileTooBig)?;
    let compressed_body = compress_to_vec_zlib(body, 9);

    push(
        writer,
        filename.as_ref(),
        &compressed_body,
        uncompressed_size,
    )
    .await
}
