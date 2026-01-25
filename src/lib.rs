// SPDX-FileCopyrightText: 2026 Manuel Quarneti <mq1@ik.me>
// SPDX-License-Identifier: GPL-3.0-only

use flate2::{Compression, write::DeflateEncoder};
use std::{
    io::{BufWriter, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    time::Duration,
};
use thiserror::Error;

const WIILOAD_PORT: u16 = 4299;
const WIILOAD_MAGIC: &[u8] = b"HAXX";
const WIILOAD_VERSION: [u8; 2] = [0, 5];
const WIILOAD_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Error, Debug)]
pub enum WiiloadError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Network error: {0}")]
    Net(#[from] std::net::AddrParseError),
    #[error("Timeout")]
    Timeout,
    #[error("File too big")]
    TryFromIntError(#[from] std::num::TryFromIntError),
}

fn push(
    mut filename: String,
    body: &[u8],
    wii_ip: impl Into<Ipv4Addr>,
    uncompressed_size: u32,
) -> Result<(), WiiloadError> {
    // Filename must end with 0x0
    if !filename.ends_with('\0') {
        filename.push('\0');
    }

    let compressed_size: u32 = body.len().try_into()?;
    let filename_len: u8 = filename.len().try_into()?;

    // Parse the address
    let wii_ip = wii_ip.into();
    let wii_addr = SocketAddr::from((wii_ip, WIILOAD_PORT));

    // Connect to the Wii via tcp
    let mut stream = {
        let stream = TcpStream::connect_timeout(&wii_addr, WIILOAD_TIMEOUT)?;
        stream.set_read_timeout(Some(WIILOAD_TIMEOUT))?;
        stream.set_write_timeout(Some(WIILOAD_TIMEOUT))?;
        BufWriter::new(stream)
    };

    // Send Wiiload header
    stream.write_all(WIILOAD_MAGIC)?;
    stream.write_all(&WIILOAD_VERSION[..])?;
    stream.write_all(&[filename_len])?;
    stream.write_all(&compressed_size.to_be_bytes())?;
    stream.write_all(&uncompressed_size.to_be_bytes())?;

    // Send the data
    stream.write_all(body)?;

    // Send arguments
    stream.write_all(filename.as_bytes())?;

    stream.flush()?;

    Ok(())
}

pub fn send(filename: String, body: &[u8], wii_ip: Ipv4Addr) -> Result<(), WiiloadError> {
    push(filename, body, wii_ip, 0)
}

pub fn compress_then_send(
    filename: String,
    body: &[u8],
    wii_ip: Ipv4Addr,
) -> Result<(), WiiloadError> {
    let uncompressed_size = body.len().try_into()?;

    let mut e = DeflateEncoder::new(Vec::new(), Compression::best());
    e.write_all(body)?;
    let compressed_body = e.finish()?;

    push(filename, &compressed_body, wii_ip, uncompressed_size)
}
