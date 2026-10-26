// SPDX-FileCopyrightText: 2026 Manuel Quarneti <mq1@ik.me>
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(clippy::all, rust_2018_idioms)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub const WIILOAD_PORT: u16 = 4299;
const EMPTY_HEADER: [u8; 16] = [b'H', b'A', b'X', b'X', 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

fn make_header(
    filename_len: usize,
    compressed_size: usize,
    uncompressed_size: usize,
) -> std::io::Result<[u8; 16]> {
    if filename_len > 254 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "filename too long",
        ));
    }

    let filename_len = filename_len as u16 + 1;
    let compressed_size = u32::try_from(compressed_size)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::FileTooLarge))?;
    let uncompressed_size = u32::try_from(uncompressed_size)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::FileTooLarge))?;

    let mut header = EMPTY_HEADER;
    header[6..8].copy_from_slice(&filename_len.to_be_bytes());
    header[8..12].copy_from_slice(&compressed_size.to_be_bytes());
    header[12..16].copy_from_slice(&uncompressed_size.to_be_bytes());

    Ok(header)
}

fn push(
    writer: &mut dyn std::io::Write,
    filename: &str,
    body: &mut dyn std::io::Read,
    compressed_size: usize,
    uncompressed_size: usize,
) -> std::io::Result<()> {
    // Send Wiiload header
    let header = make_header(filename.len(), compressed_size, uncompressed_size)?;
    writer.write_all(&header)?;

    // Send the data
    std::io::copy(body, writer)?;

    // Send filename with null terminator
    writer.write_all(filename.as_bytes())?;
    writer.write_all(&[0])?;

    writer.flush()?;
    Ok(())
}

fn push_slice(
    writer: &mut dyn std::io::Write,
    filename: &str,
    body: &[u8],
    uncompressed_size: usize,
) -> std::io::Result<()> {
    // Send Wiiload header
    let header = make_header(filename.len(), body.len(), uncompressed_size)?;
    writer.write_all(&header)?;

    // Send the data
    writer.write_all(body)?;

    // Send filename with null terminator
    writer.write_all(filename.as_bytes())?;
    writer.write_all(&[0])?;

    writer.flush()?;
    Ok(())
}

#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
#[cfg(feature = "async")]
async fn push_async(
    writer: &mut (impl futures_lite::AsyncWrite + Unpin),
    filename: &str,
    body: &mut (impl futures_lite::AsyncRead + Unpin),
    compressed_size: usize,
    uncompressed_size: usize,
) -> std::io::Result<()> {
    use futures_lite::AsyncWriteExt;

    // Send Wiiload header
    let header = make_header(filename.len(), compressed_size, uncompressed_size)?;
    writer.write_all(&header).await?;

    // Send the data
    futures_lite::io::copy(body, &mut *writer).await?;

    // Send filename with null terminator
    writer.write_all(filename.as_bytes()).await?;
    writer.write_all(&[0]).await?;

    writer.flush().await?;
    Ok(())
}

#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
#[cfg(feature = "async")]
async fn push_slice_async(
    writer: &mut (impl futures_lite::AsyncWrite + Unpin),
    filename: &str,
    body: &[u8],
    uncompressed_size: usize,
) -> std::io::Result<()> {
    use futures_lite::AsyncWriteExt;

    // Send Wiiload header
    let header = make_header(filename.len(), body.len(), uncompressed_size)?;
    writer.write_all(&header).await?;

    // Send the data
    writer.write_all(body).await?;

    // Send filename with null terminator
    writer.write_all(filename.as_bytes()).await?;
    writer.write_all(&[0]).await?;

    writer.flush().await?;
    Ok(())
}

/// Sends a file to the Wii without applying any compression.
pub fn send(
    mut writer: impl std::io::Write,
    filename: impl AsRef<str>,
    mut body: impl std::io::Read,
    size: usize,
) -> std::io::Result<()> {
    push(&mut writer, filename.as_ref(), &mut body, size, 0)
}

/// Sends a file to the Wii without applying any compression.
pub fn send_slice(
    mut writer: impl std::io::Write,
    filename: impl AsRef<str>,
    body: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    push_slice(&mut writer, filename.as_ref(), body.as_ref(), 0)
}

#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
#[cfg(feature = "async")]
/// Sends a file to the Wii without applying any compression.
pub async fn send_async(
    mut writer: impl futures_lite::AsyncWrite + Unpin,
    filename: impl AsRef<str>,
    mut body: impl futures_lite::AsyncRead + Unpin,
    size: usize,
) -> std::io::Result<()> {
    push_async(&mut writer, filename.as_ref(), &mut body, size, 0).await
}

#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
#[cfg(feature = "async")]
/// Sends a file to the Wii without applying any compression.
pub async fn send_slice_async(
    mut writer: impl futures_lite::AsyncWrite + Unpin,
    filename: impl AsRef<str>,
    body: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    push_slice_async(&mut writer, filename.as_ref(), body.as_ref(), 0).await
}

/// Compresses the file data using Zlib and then sends it to the Wii.
/// Uses deflate -9 to minimize network transfer time.
#[cfg_attr(docsrs, doc(cfg(feature = "compression")))]
#[cfg(feature = "compression")]
pub fn compress_then_send(
    mut writer: impl std::io::Write,
    filename: impl AsRef<str>,
    body: impl std::io::BufRead,
) -> std::io::Result<()> {
    use std::io::Read;

    let mut encoder = flate2::bufread::ZlibEncoder::new(body, flate2::Compression::best());

    let mut compressed_body = Vec::new();
    encoder.read_to_end(&mut compressed_body)?;

    let uncompressed_size = usize::try_from(encoder.total_in())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::FileTooLarge))?;

    push_slice(
        &mut writer,
        filename.as_ref(),
        compressed_body.as_ref(),
        uncompressed_size,
    )
}

/// Compresses the file data using Zlib and then sends it to the Wii.
/// Uses deflate -9 to minimize network transfer time.
#[cfg_attr(docsrs, doc(cfg(feature = "compression")))]
#[cfg(feature = "compression")]
pub fn compress_then_send_slice(
    mut writer: impl std::io::Write,
    filename: impl AsRef<str>,
    body: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    compress_then_send(&mut writer, filename, body.as_ref())
}

/// Compresses the file data using Zlib and then sends it to the Wii.
/// Uses deflate -9 to minimize network transfer time.
#[cfg_attr(docsrs, doc(cfg(feature = "compression-async")))]
#[cfg(feature = "compression-async")]
pub async fn compress_then_send_async(
    mut writer: impl futures_lite::AsyncWrite + Unpin,
    filename: impl AsRef<str>,
    body: impl futures_lite::AsyncBufRead + Unpin,
) -> std::io::Result<()> {
    use futures_lite::AsyncReadExt;

    let mut encoder = async_compression::futures::bufread::ZlibEncoder::with_quality(
        body,
        async_compression::Level::Best,
    );

    let mut compressed_body = Vec::new();
    encoder.read_to_end(&mut compressed_body).await?;

    let uncompressed_size = usize::try_from(encoder.total_in())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::FileTooLarge))?;

    push_slice_async(
        &mut writer,
        filename.as_ref(),
        compressed_body.as_ref(),
        uncompressed_size,
    )
    .await
}

/// Compresses the file data using Zlib and then sends it to the Wii.
/// Uses deflate -9 to minimize network transfer time.
#[cfg_attr(docsrs, doc(cfg(feature = "compression-async")))]
#[cfg(feature = "compression-async")]
pub async fn compress_then_send_slice_async(
    writer: impl futures_lite::AsyncWrite + Unpin,
    filename: impl AsRef<str>,
    body: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    compress_then_send_async(writer, filename, body.as_ref()).await
}
