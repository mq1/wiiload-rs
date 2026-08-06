// SPDX-FileCopyrightText: 2026 Manuel Quarneti <mq1@ik.me>
// SPDX-License-Identifier: MIT OR Apache-2.0

#![warn(clippy::all, rust_2018_idioms)]

#[cfg(feature = "cli")]
struct Args {
    file: String,
    wii_ip: std::net::IpAddr,
    wii_port: u16,
    compress: bool,
}

#[cfg(feature = "cli")]
fn parse_args() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;

    let mut file = None;
    let mut wii_ip = None;
    let mut compress = false;
    let mut wii_port = wiiload::WIILOAD_PORT;

    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Short('i') | Long("wii-ip") => {
                wii_ip = Some(parser.value()?.parse()?);
            }
            Short('p') | Long("wii-port") => {
                wii_port = parser.value()?.parse()?;
            }
            Short('c') | Long("compress") => {
                compress = true;
            }
            Value(val) => {
                file = Some(val.string()?);
            }
            Short('h') | Long("help") => {
                println!("Usage: wiiload [-i|--wii-ip=IP] [-p|--wii-port=PORT] [--compress] FILE");
                std::process::exit(0);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    Ok(Args {
        file: file.ok_or("Please specify a file")?,
        wii_ip: wii_ip.ok_or("Please specify a wii ip")?,
        compress,
        wii_port,
    })
}

#[cfg(feature = "cli")]
fn main() -> Result<(), lexopt::Error> {
    let args = parse_args()?;

    futures::executor::block_on(async move {
        let file_path = std::path::Path::new(&args.file);
        let body = std::fs::read(file_path).unwrap();
        let filename = file_path.file_name().unwrap().to_str().unwrap().to_string();

        let conn = std::net::TcpStream::connect((args.wii_ip, args.wii_port)).unwrap();
        let mut conn = futures::io::AllowStdIo::new(conn);

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
