// SPDX-FileCopyrightText: 2026 Manuel Quarneti <mq1@ik.me>
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(clippy::all, rust_2018_idioms)]

use futures_lite::{AsyncWrite, AsyncWriteExt};
use thiserror::Error;

pub const WIILOAD_PORT: u16 = 4299;
const WIILOAD_MAGIC: [u8; 4] = *b"HAXX";
const WIILOAD_VERSION: [u8; 2] = [0, 5];
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

fn make_header(filename_len: u16, compressed_size: u32, uncompressed_size: u32) -> [u8; 16] {
    let mut buf = [0u8; 16];

    buf[0..4].copy_from_slice(&WIILOAD_MAGIC);
    buf[4..6].copy_from_slice(&WIILOAD_VERSION);
    buf[6..8].copy_from_slice(&filename_len.to_be_bytes());
    buf[8..12].copy_from_slice(&compressed_size.to_be_bytes());
    buf[12..16].copy_from_slice(&uncompressed_size.to_be_bytes());

    buf
}

async fn push<W: AsyncWrite + Unpin>(
    writer: &mut W,
    mut filename: String,
    body: &[u8],
    uncompressed_size: u32,
) -> Result<(), WiiloadError> {
    let compressed_size: u32 = body
        .len()
        .try_into()
        .map_err(|_| WiiloadError::FileTooBig)?;

    let filename_len = filename
        .len()
        .try_into()
        .map_err(|_| WiiloadError::FileNameTooLong)?;

    // Send Wiiload header
    let header = make_header(filename_len, compressed_size, uncompressed_size);
    writer.write_all(&header).await?;

    // Send the data
    for chunk in body.chunks(CHUNK_SIZE) {
        writer.write_all(chunk).await?;
    }

    // Send filename with null terminator
    if !filename.ends_with('\0') {
        filename.push('\0');
    }
    writer.write_all(filename.as_bytes()).await?;

    Ok(())
}

/// Sends a file to the Wii without applying any compression.
pub async fn send<W: AsyncWrite + Unpin>(
    writer: &mut W,
    filename: impl Into<String>,
    body: impl AsRef<[u8]>,
) -> Result<(), WiiloadError> {
    push(writer, filename.into(), body.as_ref(), 0).await
}

/// Compresses the file data using Zlib and then sends it to the Wii.
/// Uses deflate -9 to minimize network transfer time.
#[cfg(feature = "compression")]
pub async fn compress_then_send<W: AsyncWrite + Unpin>(
    writer: &mut W,
    filename: impl Into<String>,
    body: impl AsRef<[u8]>,
) -> Result<(), WiiloadError> {
    let body = body.as_ref();

    let uncompressed_size = body
        .len()
        .try_into()
        .map_err(|_| WiiloadError::FileTooBig)?;

    let compressed_body = miniz_oxide::deflate::compress_to_vec_zlib(body, 9);

    push(writer, filename.into(), &compressed_body, uncompressed_size).await
}
