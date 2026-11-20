// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! `qsmcsim`: the prototype's managed systems on the network, one port
//! each from `--port`, told what to do a line at a time on standard
//! input (`help` lists the commands).

use std::io::BufRead as _;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut address = "127.0.0.1".to_string();
    let mut port: u16 = 2300;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--address" => address = args.next().unwrap_or(address),
            "--port" => port = args.next().and_then(|p| p.parse().ok()).unwrap_or(port),
            _ => {
                eprintln!("usage: qsmcsim [--address A] [--port FIRST]");
                std::process::exit(2);
            }
        }
    }
    let systems = sim::fixture::prototype();
    for (i, s) in systems.iter().enumerate() {
        let p = port + u16::try_from(i).unwrap_or(0);
        match sim::net::listen(s.clone(), &format!("{address}:{p}")) {
            Ok(at) => {
                let s = sim::lock(s);
                println!("{} {at} {}", s.name(), s.identity.fingerprint().hex());
            }
            Err(e) => {
                eprintln!("{address}:{p}: {e}");
                std::process::exit(1);
            }
        }
    }
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        if line.trim() == "quit" {
            break;
        }
        println!("{}", sim::control::command(&systems, &line));
    }
}
