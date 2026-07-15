// SPDX-FileCopyrightText: 2026 Manuel Quarneti <mq1@ik.me>
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(clippy::all, rust_2018_idioms)]

#[cfg(feature = "cli")]
struct Args {
    file: String,
    wii_ip: String,
    compress: bool,
}

#[cfg(feature = "cli")]
fn parse_args() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut wii_ip = None;
    let mut compress = false;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Long("wii-ip") => {
                wii_ip = Some(parser.value()?.string()?);
            }
            Long("compress") => {
                compress = true;
            }
            Value(val) => {
                file = Some(val.string()?);
            }
            Short('h') | Long("help") => {
                println!("Usage: wiiload [--wii-ip=IP] [--compress] FILE");
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Args {
        file: file.ok_or("Please specify a file")?,
        wii_ip: wii_ip.ok_or("Please specify a wii ip")?,
        compress,
    })
}

#[cfg(feature = "cli")]
fn main() -> Result<(), lexopt::Error> {
    use async_net::TcpStream;
    use std::{
        fs,
        net::{Ipv4Addr, SocketAddr},
        path::Path,
    };

    let args = parse_args()?;
    let file_path = Path::new(&args.file);
    let body = fs::read(file_path).unwrap();
    let filename = file_path.file_name().unwrap().to_str().unwrap().to_string();
    let wii_ip: Ipv4Addr = args.wii_ip.parse().unwrap();

    futures_lite::future::block_on(async move {
        let addr = SocketAddr::from((wii_ip, wiiload::WIILOAD_PORT));
        let mut conn = TcpStream::connect(addr).await.unwrap();

        if args.compress {
            #[cfg(feature = "compression")]
            {
                println!("Compressing and sending file...");
                wiiload::compress_then_send(&mut conn, filename, &body)
                    .await
                    .unwrap();
            }
            #[cfg(not(feature = "compression"))]
            {
                println!(
                    "Compression not enabled! Please add the `compression` feature to enable it."
                );
            }
        } else {
            println!("Sending file...");
            wiiload::send(&mut conn, filename, &body).await.unwrap();
        }
    });

    Ok(())
}

#[cfg(not(feature = "cli"))]
fn main() {
    println!("Please add the `cli` feature to enable the CLI");
}
