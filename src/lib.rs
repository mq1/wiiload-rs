// SPDX-FileCopyrightText: 2026 Manuel Quarneti <mq1@ik.me>
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(clippy::all, rust_2018_idioms)]

use futures_lite::{AsyncWrite, AsyncWriteExt, io::BufWriter};
use thiserror::Error;

pub const WIILOAD_PORT: u16 = 4299;
const WIILOAD_MAGIC: &[u8] = b"HAXX";
const WIILOAD_VERSION: [u8; 3] = [0, 5, 0];

#[derive(Error, Debug)]
pub enum WiiloadError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Network error: {0}")]
    Net(#[from] std::net::AddrParseError),
    #[error("Timeout")]
    Timeout,
    #[error("File too big")]
    FileTooBig,
    #[error("Filename too long")]
    FileNameTooLong,
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

    let filename_len: u8 = filename
        .len()
        .try_into()
        .map_err(|_| WiiloadError::FileNameTooLong)?;

    // Buffered writes
    let mut writer = BufWriter::new(writer);

    // Send Wiiload header
    writer.write_all(WIILOAD_MAGIC).await?;
    writer.write_all(&WIILOAD_VERSION[..]).await?;
    writer.write_all(&[filename_len]).await?;
    writer.write_all(&compressed_size.to_be_bytes()).await?;
    writer.write_all(&uncompressed_size.to_be_bytes()).await?;

    // Send the data
    writer.write_all(body).await?;

    // Send arguments
    writer.write_all(filename.as_bytes()).await?;
    if !filename.ends_with('\0') {
        writer.write_all(&[0]).await?;
    }

    writer.flush().await?;

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
