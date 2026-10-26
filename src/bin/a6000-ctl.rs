#![forbid(unsafe_code)]
//! `a6000-ctl` — Sovereign CLI for the A6000 Constellation Orchestrator (`49.13.101.247:8790` / `https://cheet.is/a6000`)
//!
//! Subcommands:
//! - `a6000-ctl status`         -> Subsystem A (`GET /a6000/api/status`)
//! - `a6000-ctl fleet`          -> Subsystem B (`POST /a6000/api/fleet/probe`)
//! - `a6000-ctl sync-repos`     -> Subsystem B (`POST /a6000/api/fleet/sync-repos`)
//! - `a6000-ctl g8t-bench`      -> Subsystem C (`POST /a6000/api/g8t/orchestrate`)
//! - `a6000-ctl toss`           -> Subsystem D (`POST /a6000/api/toss/execute`)
//! - `a6000-ctl diary-pulse`    -> Subsystem D (`POST /a6000/api/diary/pulse`)
//! - `a6000-ctl backup-verify`  -> Subsystem E (`POST /a6000/api/backup/verify`)
//! - `a6000-ctl improve [repo]` -> Subsystem F (`POST /a6000/api/improve/cycle`)
//! - `a6000-ctl improve-status` -> Subsystem F (`GET /a6000/api/improve/status`)
//! - `a6000-ctl borg-sync [ow]` -> Subsystem G (`POST /a6000/api/borg/sync`)
//! - `a6000-ctl borg-status`    -> Subsystem G (`GET /a6000/api/borg/status`)

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::Duration;

fn request_local_daemon(method: &str, path: &str) -> Option<String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8790));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(250)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
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
        .args(["-sk", "--max-time", "30", "-X", method, &url])
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
    let remote_cmd = format!("curl -s -X {} 'http://127.0.0.1:8790{}'", method, path);
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

fn sanitize_arg(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect()
}

fn print_usage() {
    println!("a6000-ctl — 100% Pure-Rust A6000 Sovereign Constellation CLI (49.13.101.247 · RTX 6000 Ada)");
    println!("Strict 8b·is No-Float Law: all values are integers, reduced p/q rationals, or {{-1,0,+1}} trits.");
    println!();
    println!("Usage: a6000-ctl <command> [arg]");
    println!("Commands:");
    println!("  status              Subsystem A: RTX 6000 Ada GPU (49140 MiB) + Local AI/Audio Stack (:8786/:8787/:11434/:8795)");
    println!("  fleet               Subsystem B: Probe 9 Hetzner Servers + ns3·i1·is Mesh & Storage Vaults");
    println!("  sync-repos          Subsystem B: Sync all 13 updated 8b-is GitHub repos onto /data/8b-is");
    println!("  g8t-bench           Subsystem C: Benchmark g8t·is Capsules, Cast AyeAttestation Votes & Snap Circuit");
    println!("  toss                Subsystem D: Execute Peter's 9-Step Tokyo Monozukuri TOSS Spiral ([0,0] return)");
    println!("  diary-pulse         Subsystem D: Pulse gemini-diary·8b·is + Check WhatsApp 'little dude(s) and Angels'");
    println!("  backup-verify       Subsystem E: Verify 321-File 8b-is-local-state-backup-20261025·tar·zst on A6000 & RAID6");
    println!("  improve [repo]      Subsystem F: Trigger RTX 6000 Ada Code Audit, Synthesis & rustc Benchmark Cycle");
    println!("  improve-status      Subsystem F: Inspect Continuous Improver Summary & Recent 64-Slot Ring Records");
    println!("  borg-sync [owner]   Subsystem G: Pure-Rust sync-github onto 160 TB VLAN 4010 Vault + Borg Assimilator");
    println!("  borg-status         Subsystem G: Inspect Hetzner VLAN 4010 (160 TB) Link & Borg-Assimilated Capsules");
    println!("  domains [reconcile] Subsystem H: Namecheap (lumisst) Registrar & ns1..ns4·i1·is (i1-dns) Zone Reconciler");
    println!("  commitments         Subsystem I: Inspect MEM|8 16-Byte Normative Commitments & 3-Op Continuation Guard");
    println!("  commitment-transition [id] [state] [justification] [sig]  Subsystem I: Transition MEM|8 Commitment");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let prog = args.first().map(String::as_str).unwrap_or("a6000-ctl");

    if prog.ends_with("sync-github") {
        let owner = args
            .get(1)
            .map(|s| sanitize_arg(s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "8b-is".to_string());
        let path = format!("/a6000/api/borg/sync?owner={}", owner);
        println!("{}", execute_ctl_request("POST", &path));
        return;
    }

    let cmd = args.get(1).map(String::as_str).unwrap_or("status");

    let (method, path): (&str, String) = match cmd {
        "status" => ("GET", "/a6000/api/status".to_string()),
        "fleet" | "probe" => ("POST", "/a6000/api/fleet/probe".to_string()),
        "sync-repos" | "sync" => ("POST", "/a6000/api/fleet/sync-repos".to_string()),
        "g8t-bench" | "g8t" => ("POST", "/a6000/api/g8t/orchestrate".to_string()),
        "toss" => ("POST", "/a6000/api/toss/execute".to_string()),
        "diary-pulse" | "diary" => ("POST", "/a6000/api/diary/pulse".to_string()),
        "backup-verify" | "backup" => ("POST", "/a6000/api/backup/verify".to_string()),
        "improve" | "improve-cycle" => {
            if let Some(repo) = args.get(2).map(|s| sanitize_arg(s)).filter(|s| !s.is_empty()) {
                ("POST", format!("/a6000/api/improve/cycle?repo={}", repo))
            } else {
                ("POST", "/a6000/api/improve/cycle".to_string())
            }
        }
        "improve-status" => ("GET", "/a6000/api/improve/status".to_string()),
        "borg-sync" | "sync-github" | "assimilate" => {
            let owner = args
                .get(2)
                .map(|s| sanitize_arg(s))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "8b-is".to_string());
            ("POST", format!("/a6000/api/borg/sync?owner={}", owner))
        }
        "borg-status" | "borg" => ("GET", "/a6000/api/borg/status".to_string()),
        "domains" => {
            let sub = args.get(2).map(String::as_str).unwrap_or("");
            if sub == "reconcile" {
                ("POST", "/api/a6000/domains/reconcile".to_string())
            } else if sub == "check" {
                let d = args.get(3).map(|s| sanitize_arg(s)).unwrap_or_else(|| "sovereign-8b.is".to_string());
                ("GET", format!("/api/a6000/domains?check={}", d))
            } else if sub == "create" {
                let d = args.get(3).map(|s| sanitize_arg(s)).unwrap_or_else(|| "sovereign-8b.is".to_string());
                ("POST", format!("/api/a6000/domains/reconcile?create={}", d))
            } else {
                ("GET", "/api/a6000/domains".to_string())
            }
        }
        "domains-reconcile" => ("POST", "/api/a6000/domains/reconcile".to_string()),
        "commitments" => ("GET", "/api/a6000/commitments".to_string()),
        "commitment-transition" | "transition" => {
            let id = args.get(2).map(|s| sanitize_arg(s)).unwrap_or_default();
            let st = args.get(3).map(|s| sanitize_arg(s)).unwrap_or_default();
            let just = args.get(4).map(|s| sanitize_arg(s)).unwrap_or_default();
            let sig = args.get(5).map(|s| sanitize_arg(s)).unwrap_or_default();
            if !id.is_empty() && !st.is_empty() {
                (
                    "POST",
                    format!(
                        "/api/a6000/commitments/transition?id={}&target_state={}&justification={}&counterparty_sig_hash={}",
                        id, st, just, sig
                    ),
                )
            } else {
                ("POST", "/api/a6000/commitments/transition".to_string())
            }
        }
        "-h" | "--help" | "help" => {
            print_usage();
            return;
        }
        _ => {
            print_usage();
            std::process::exit(1);
        }
    };

    let json = execute_ctl_request(method, &path);
    println!("{}", json);
}

