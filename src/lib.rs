// SPDX-FileCopyrightText: 2026 Manuel Quarneti <mq1@ik.me>
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(clippy::all, rust_2018_idioms)]

pub const WIILOAD_PORT: u16 = 4299;
const WIILOAD_MAGIC: [u8; 4] = *b"HAXX";
const WIILOAD_VERSION: [u8; 2] = [0, 5];
const CHUNK_SIZE: usize = 1024 * 128;

#[derive(thiserror::Error, Debug, Clone)]
pub enum WiiloadError {
    #[error("I/O error: {0}")]
    Io(std::io::ErrorKind),

    #[error("File > 4 GiB")]
    FileTooBig,

    #[error("Filename > 255")]
    FileNameTooLong,
}

impl From<std::io::Error> for WiiloadError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.kind())
    }
}

fn make_header(
    filename_len: usize,
    compressed_size: usize,
    uncompressed_size: usize,
) -> Result<[u8; 16], WiiloadError> {
    if filename_len > 255 {
        return Err(WiiloadError::FileNameTooLong);
    }

    let filename_len = filename_len as u16;
    let compressed_size = u32::try_from(compressed_size).map_err(|_| WiiloadError::FileTooBig)?;
    let uncompressed_size =
        u32::try_from(uncompressed_size).map_err(|_| WiiloadError::FileTooBig)?;

    let mut buf = [0u8; 16];

    buf[0..4].copy_from_slice(&WIILOAD_MAGIC);
    buf[4..6].copy_from_slice(&WIILOAD_VERSION);
    buf[6..8].copy_from_slice(&filename_len.to_be_bytes());
    buf[8..12].copy_from_slice(&compressed_size.to_be_bytes());
    buf[12..16].copy_from_slice(&uncompressed_size.to_be_bytes());

    Ok(buf)
}

fn null_terminated_filename(filename: &str) -> ([u8; 256], usize) {
    let filename = filename.as_bytes();

    let mut buf = [0u8; 256];
    buf[0..filename.len()].copy_from_slice(filename);

    (buf, filename.len() + 1)
}

fn push<R: std::io::Read, W: std::io::Write>(
    writer: &mut W,
    filename: &str,
    body: &mut R,
    compressed_size: usize,
    uncompressed_size: usize,
) -> Result<(), WiiloadError> {
    use std::io::Write;

    // Send Wiiload header
    let header = make_header(filename.len(), compressed_size, uncompressed_size)?;
    writer.write_all(&header)?;

    // Send the data
    let mut writer = std::io::BufWriter::with_capacity(CHUNK_SIZE, writer);
    std::io::copy(body, &mut writer)?;

    // Send filename with null terminator
    let (filename, len) = null_terminated_filename(filename);
    writer.write_all(&filename[0..len])?;

    Ok(())
}

#[cfg(feature = "async")]
async fn push_async<R: futures_lite::AsyncRead + Unpin, W: futures_lite::AsyncWrite + Unpin>(
    writer: &mut W,
    filename: &str,
    body: &mut R,
    compressed_size: usize,
    uncompressed_size: usize,
) -> Result<(), WiiloadError> {
    use futures_lite::AsyncWriteExt;

    // Send Wiiload header
    let header = make_header(filename.len(), compressed_size, uncompressed_size)?;
    writer.write_all(&header).await?;

    // Send the data
    let mut writer = futures_lite::io::BufWriter::with_capacity(CHUNK_SIZE, writer);
    futures_lite::io::copy(body, &mut writer).await?;

    // Send filename with null terminator
    let (filename, len) = null_terminated_filename(filename);
    writer.write_all(&filename[0..len]).await?;

    Ok(())
}

/// Sends a file to the Wii without applying any compression.
pub fn send<R: std::io::Read, W: std::io::Write>(
    writer: &mut W,
    filename: impl AsRef<str>,
    body: &mut R,
    size: usize,
) -> Result<(), WiiloadError> {
    push(writer, filename.as_ref(), body, size, 0)
}

#[cfg(feature = "async")]
/// Sends a file to the Wii without applying any compression.
pub async fn send_async<R: futures_lite::AsyncRead + Unpin, W: futures_lite::AsyncWrite + Unpin>(
    writer: &mut W,
    filename: impl AsRef<str>,
    body: &mut R,
    size: usize,
) -> Result<(), WiiloadError> {
    push_async(writer, filename.as_ref(), body, size, 0).await
}

/// Compresses the file data using Zlib and then sends it to the Wii.
/// Uses deflate -9 to minimize network transfer time.
#[cfg(feature = "compression")]
pub fn compress_then_send<R: std::io::Read, W: std::io::Write>(
    writer: &mut W,
    filename: impl AsRef<str>,
    body: &mut R,
) -> Result<(), WiiloadError> {
    use std::io::Seek;

    let mut tmp =
        flate2::write::ZlibEncoder::new(tempfile::tempfile()?, flate2::Compression::best());
    let uncompressed_size =
        usize::try_from(std::io::copy(body, &mut tmp)?).map_err(|_| WiiloadError::FileTooBig)?;
    let mut tmp = tmp.finish()?;
    tmp.rewind()?;

    let compressed_size =
        usize::try_from(tmp.metadata()?.len()).map_err(|_| WiiloadError::FileTooBig)?;

    push(
        writer,
        filename.as_ref(),
        &mut tmp,
        compressed_size,
        uncompressed_size,
    )
}

/// Compresses the file data using Zlib and then sends it to the Wii.
/// Uses deflate -9 to minimize network transfer time.
#[cfg(all(feature = "compression", feature = "async"))]
pub async fn compress_then_send_async<
    R: futures_lite::AsyncRead + Unpin,
    W: futures_lite::AsyncWrite + Unpin,
>(
    writer: &mut W,
    filename: impl AsRef<str>,
    body: &mut R,
) -> Result<(), WiiloadError> {
    use futures_lite::AsyncSeekExt;

    let mut tmp =
        flate2::write::ZlibEncoder::new(tempfile::tempfile()?, flate2::Compression::best());
    let uncompressed_size = usize::try_from(std::io::copy(
        &mut futures_lite::io::BlockOn::new(body),
        &mut tmp,
    )?)
    .map_err(|_| WiiloadError::FileTooBig)?;
    let tmp = tmp.finish()?;

    let mut tmp = async_fs::File::from(tmp);
    tmp.seek(std::io::SeekFrom::Start(0)).await?;

    let compressed_size =
        usize::try_from(tmp.metadata().await?.len()).map_err(|_| WiiloadError::FileTooBig)?;

    push_async(
        writer,
        filename.as_ref(),
        &mut tmp,
        compressed_size,
        uncompressed_size,
    )
    .await
}
