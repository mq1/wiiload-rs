// SPDX-FileCopyrightText: 2026 Manuel Quarneti <mq1@ik.me>
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(clippy::all, rust_2018_idioms)]

pub const WIILOAD_PORT: u16 = 4299;
const WIILOAD_MAGIC: [u8; 4] = *b"HAXX";
const WIILOAD_VERSION: [u8; 2] = [0, 5];
const CHUNK_SIZE: usize = 1024 * 128;

#[derive(thiserror::Error, Debug)]
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

fn make_header(filename_len: usize, compressed_size: usize, uncompressed_size: u32) -> [u8; 16] {
    let mut buf = [0u8; 16];

    buf[0..4].copy_from_slice(&WIILOAD_MAGIC);
    buf[4..6].copy_from_slice(&WIILOAD_VERSION);
    buf[6..8].copy_from_slice(&(filename_len as u16).to_be_bytes());
    buf[8..12].copy_from_slice(&(compressed_size as u32).to_be_bytes());
    buf[12..16].copy_from_slice(&uncompressed_size.to_be_bytes());

    buf
}

fn null_terminated_filename(filename: &str) -> ([u8; 256], usize) {
    let filename = filename.as_bytes();

    let mut buf = [0u8; 256];
    buf[0..filename.len()].copy_from_slice(filename);

    (buf, filename.len() + 1)
}

fn push<W: std::io::Write>(
    writer: &mut W,
    filename: &str,
    body: &[u8],
    uncompressed_size: u32,
) -> Result<(), WiiloadError> {
    // Send Wiiload header
    let header = make_header(filename.len(), body.len(), uncompressed_size);
    writer.write_all(&header)?;

    // Send the data
    for chunk in body.chunks(CHUNK_SIZE) {
        writer.write_all(chunk)?;
    }

    // Send filename with null terminator
    let (filename, len) = null_terminated_filename(filename);
    writer.write_all(&filename[0..len])?;

    Ok(())
}

#[cfg(feature = "async")]
async fn push_async<W: futures_lite::AsyncWrite + Unpin>(
    writer: &mut W,
    filename: &str,
    body: &[u8],
    uncompressed_size: u32,
) -> Result<(), WiiloadError> {
    use futures_lite::AsyncWriteExt;

    // Send Wiiload header
    let header = make_header(filename.len(), body.len(), uncompressed_size);
    writer.write_all(&header).await?;

    // Send the data
    for chunk in body.chunks(CHUNK_SIZE) {
        writer.write_all(chunk).await?;
    }

    // Send filename with null terminator
    let (filename, len) = null_terminated_filename(filename);
    writer.write_all(&filename[0..len]).await?;

    Ok(())
}

fn check(filename: &str, body: &[u8]) -> Result<(), WiiloadError> {
    if filename.len() > 255 {
        return Err(WiiloadError::FileNameTooLong);
    }

    if body.len() > u32::MAX as usize {
        return Err(WiiloadError::FileTooBig);
    }

    Ok(())
}

/// Sends a file to the Wii without applying any compression.
pub fn send<W: std::io::Write>(
    writer: &mut W,
    filename: impl AsRef<str>,
    body: impl AsRef<[u8]>,
) -> Result<(), WiiloadError> {
    let filename = filename.as_ref();
    let body = body.as_ref();

    check(filename, body)?;
    push(writer, filename, body, 0)
}

#[cfg(feature = "async")]
/// Sends a file to the Wii without applying any compression.
pub async fn send_async<W: futures_lite::AsyncWrite + Unpin>(
    writer: &mut W,
    filename: impl AsRef<str>,
    body: impl AsRef<[u8]>,
) -> Result<(), WiiloadError> {
    let filename = filename.as_ref();
    let body = body.as_ref();

    check(filename, body)?;
    push_async(writer, filename, body, 0).await
}

/// Compresses the file data using Zlib and then sends it to the Wii.
/// Uses deflate -9 to minimize network transfer time.
#[cfg(feature = "compression")]
pub fn compress_then_send<W: std::io::Write>(
    writer: &mut W,
    filename: impl AsRef<str>,
    body: impl AsRef<[u8]>,
) -> Result<(), WiiloadError> {
    let filename = filename.as_ref();
    let body = body.as_ref();

    check(filename, body)?;

    let uncompressed_size = body.len() as u32;

    let compressed_body = miniz_oxide::deflate::compress_to_vec_zlib(body, 9);

    push(writer, filename, &compressed_body, uncompressed_size)
}

/// Compresses the file data using Zlib and then sends it to the Wii.
/// Uses deflate -9 to minimize network transfer time.
#[cfg(all(feature = "compression", feature = "async"))]
pub async fn compress_then_send_async<W: futures_lite::AsyncWrite + Unpin>(
    writer: &mut W,
    filename: impl AsRef<str>,
    body: impl AsRef<[u8]>,
) -> Result<(), WiiloadError> {
    let filename = filename.as_ref();
    let body = body.as_ref();

    check(filename, body)?;

    let uncompressed_size = body.len() as u32;

    let compressed_body = miniz_oxide::deflate::compress_to_vec_zlib(body, 9);

    push_async(writer, filename.into(), &compressed_body, uncompressed_size).await
}
