#![forbid(unsafe_code)]
//! `a6000-ctl` — Sovereign CLI for the A6000 Constellation Orchestrator (`49.13.101.247:8790` / `https://cheet.is/a6000`)
//!
//! Subcommands:
//! - `a6000-ctl status`        -> Subsystem A (`GET /a6000/api/status`)
//! - `a6000-ctl fleet`         -> Subsystem B (`POST /a6000/api/fleet/probe`)
//! - `a6000-ctl sync-repos`    -> Subsystem B (`POST /a6000/api/fleet/sync-repos`)
//! - `a6000-ctl g8t-bench`     -> Subsystem C (`POST /a6000/api/g8t/orchestrate`)
//! - `a6000-ctl toss`          -> Subsystem D (`POST /a6000/api/toss/execute`)
//! - `a6000-ctl diary-pulse`   -> Subsystem D (`POST /a6000/api/diary/pulse`)
//! - `a6000-ctl backup-verify` -> Subsystem E (`POST /a6000/api/backup/verify`)

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::Duration;

fn request_local_daemon(method: &str, path: &str) -> Option<String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8790));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(250)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let req = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1:8790\r\nUser-Agent: a6000-ctl/1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        method, path
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).ok()?;
    let text = String::from_utf8_lossy(&resp);
    let body = text.split("\r\n\r\n").nth(1)?.trim();
    if body.starts_with('{') {
        Some(body.to_string())
    } else {
        None
    }
}

fn request_remote_https(method: &str, path: &str) -> Option<String> {
    let base = std::env::var("A6000_URL").unwrap_or_else(|_| "https://cheet.is".to_string());
    let url = format!("{}{}", base.trim_end_matches('/'), path);
    let out = Command::new("curl")
        .args(["-sk", "--max-time", "15", "-X", method, &url])
        .output()
        .ok()?;
    if out.status.success() {
        let body = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if body.starts_with('{') && body.contains("\"ok\":true") {
            return Some(body);
        }
    }
    None
}

fn request_ssh_fallback(method: &str, path: &str) -> Option<String> {
    let remote_cmd = format!("curl -s -X {} http://127.0.0.1:8790{}", method, path);
    let out = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=5",
            "root@49.13.101.247",
            &remote_cmd,
        ])
        .output()
        .ok()?;
    if out.status.success() {
        let body = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if body.starts_with('{') {
            return Some(body);
        }
    }
    None
}

fn execute_ctl_request(method: &str, path: &str) -> String {
    if let Some(local) = request_local_daemon(method, path) {
        return local;
    }
    if let Some(https) = request_remote_https(method, path) {
        return https;
    }
    if let Some(ssh) = request_ssh_fallback(method, path) {
        return ssh;
    }
    "{\"ok\":false,\"error\":\"unreachable\",\"float_count\":0}".to_string()
}

fn print_usage() {
    println!("a6000-ctl — 100% Pure-Rust A6000 Sovereign Constellation CLI (49.13.101.247 · RTX 6000 Ada)");
    println!("Strict 8b·is No-Float Law: all values are integers, reduced p/q rationals, or {{-1,0,+1}} trits.");
    println!();
    println!("Usage: a6000-ctl <command>");
    println!("Commands:");
    println!("  status         Subsystem A: RTX 6000 Ada GPU (49140 MiB) + Local AI/Audio Stack (:8786/:8787/:11434/:8795)");
    println!("  fleet          Subsystem B: Probe 9 Hetzner Servers + ns3·i1·is Mesh & Storage Vaults");
    println!("  sync-repos     Subsystem B: Sync all 13 updated 8b-is GitHub repos onto /data/8b-is");
    println!("  g8t-bench      Subsystem C: Benchmark g8t·is Capsules, Cast AyeAttestation Votes & Snap Circuit");
    println!("  toss           Subsystem D: Execute Peter's 9-Step Tokyo Monozukuri TOSS Spiral ([0,0] return)");
    println!("  diary-pulse    Subsystem D: Pulse gemini-diary·8b·is + Check WhatsApp 'little dude(s) and Angels'");
    println!("  backup-verify  Subsystem E: Verify 321-File 8b-is-local-state-backup-20261025·tar·zst on A6000 & RAID6");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("status");

    let (method, path) = match cmd {
        "status" => ("GET", "/a6000/api/status"),
        "fleet" | "probe" => ("POST", "/a6000/api/fleet/probe"),
        "sync-repos" | "sync" => ("POST", "/a6000/api/fleet/sync-repos"),
        "g8t-bench" | "g8t" => ("POST", "/a6000/api/g8t/orchestrate"),
        "toss" => ("POST", "/a6000/api/toss/execute"),
        "diary-pulse" | "diary" => ("POST", "/a6000/api/diary/pulse"),
        "backup-verify" | "backup" => ("POST", "/a6000/api/backup/verify"),
        "-h" | "--help" | "help" => {
            print_usage();
            return;
        }
        _ => {
            print_usage();
            std::process::exit(1);
        }
    };

    let json = execute_ctl_request(method, path);
    println!("{}", json);
}
