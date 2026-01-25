// SPDX-FileCopyrightText: 2026 Manuel Quarneti <mq1@ik.me>
// SPDX-License-Identifier: GPL-3.0-only

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
    use std::{fs, path::Path};

    let args = parse_args()?;
    let file_path = Path::new(&args.file);
    let body = fs::read(file_path).unwrap();
    let filename = file_path.file_name().unwrap().to_str().unwrap().to_string();
    let wii_ip = args.wii_ip.parse().unwrap();

    if args.compress {
        println!("Compressing and sending file...");
        wiiload::compress_then_send(filename, &body, wii_ip).unwrap();
    } else {
        println!("Sending file...");
        wiiload::send(filename, &body, wii_ip).unwrap();
    }

    Ok(())
}

#[cfg(not(feature = "cli"))]
fn main() {
    compile_error!("Please add the `cli` feature to enable the CLI");
}
