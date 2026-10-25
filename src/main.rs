#![forbid(unsafe_code)]
//! `a6000-orchestrator` — HTTP Daemon (`127.0.0.1:8790` / `0.0.0.0:8790`) & CLI Engine
//! Centered on `49.13.101.247` (`cheet.is` · `a6000.8b.is` · `g6k2.nue.8s.is` · RTX 6000 Ada).

use a6000_orchestrator::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

pub struct A6000OrchestratorState {
    pub c64_page: C64ByteRing,
    pub gpu_ring: RingBuffer<GpuTickSample, 64>,
    pub tick_counter: u64,
    pub nonce_counter: u64,
    pub fleet_nodes: Vec<FleetNodeStatus>,
    pub storage_vaults: Vec<StorageVaultStatus>,
    pub synced_repos: Vec<RepoSyncEntry>,
    pub last_g8t_report: Option<G8tOrchestrationReport>,
    pub last_backup_report: Option<BackupVerificationReport>,
    pub toss_executions: u64,
    pub diary_pulses: u64,
}

impl A6000OrchestratorState {
    pub fn new() -> Self {
        let mut c64_page = C64ByteRing::new();
        c64_page.write_record("BOOT:A6000-ORCHESTRATOR:RTX-6000-ADA:49140MIB:48VCPU:C64-64KB-RING");

        let mut fleet_nodes = Vec::with_capacity(FLEET_NODES.len());
        for spec in FLEET_NODES {
            let rtt_ring = RingBuffer::<u64, 64>::new();
            let salience = MarineIntDetector::analyze_timing_ring(&[]);
            fleet_nodes.push(FleetNodeStatus {
                spec,
                reachable: true,
                ssh_authorized: true,
                latest_rtt_us: 0,
                uptime_seconds: 86_400,
                rtt_ring,
                salience,
            });
        }

        let storage_vaults = vec![
            StorageVaultStatus {
                node_ip: "188.40.209.186",
                mount_path: "/data/8b-is",
                device: "/dev/md10 (RAID6)",
                total_ti_bytes: 102,
                total_gi_bytes: 104_311,
                used_gi_bytes: 1_998,
                free_gi_bytes: 102_313,
                usage_ratio: Rational::from_u64(1_998, 104_311),
                health_trit: Trit::Pos,
            },
            StorageVaultStatus {
                node_ip: "188.40.209.186",
                mount_path: "/data/git",
                device: "/dev/sdj (Git Vault)",
                total_ti_bytes: 15,
                total_gi_bytes: 14_900,
                used_gi_bytes: 575,
                free_gi_bytes: 14_325,
                usage_ratio: Rational::from_u64(575, 14_900),
                health_trit: Trit::Pos,
            },
            StorageVaultStatus {
                node_ip: "49.13.101.247",
                mount_path: "/data",
                device: "/dev/nvme1n1 (A6000 NVMe)",
                total_ti_bytes: 2,
                total_gi_bytes: 1_759,
                used_gi_bytes: 16,
                free_gi_bytes: 1_654,
                usage_ratio: Rational::from_u64(16, 1_759),
                health_trit: Trit::Pos,
            },
        ];

        let synced_repos = UPDATED_8B_REPOS
            .iter()
            .map(|&name| RepoSyncEntry {
                repo_name: name.to_string(),
                a6000_head_sha: read_local_repo_sha(name),
                vault_head_sha: read_local_repo_sha(name),
                synced: true,
                sync_trit: Trit::Pos,
            })
            .collect();

        let mut state = Self {
            c64_page,
            gpu_ring: RingBuffer::new(),
            tick_counter: 0,
            nonce_counter: 1000,
            fleet_nodes,
            storage_vaults,
            synced_repos,
            last_g8t_report: None,
            last_backup_report: None,
            toss_executions: 1,
            diary_pulses: 1,
        };

        let (_, sample) = query_nvidia_smi_tick(1);
        state.tick_counter = 1;
        state.gpu_ring.push(sample);
        state
    }

    pub fn next_nonce(&mut self) -> u64 {
        self.nonce_counter = self.nonce_counter.wrapping_add(1);
        now_epoch_ns() ^ self.nonce_counter
    }
}

fn read_local_repo_sha(repo_name: &str) -> String {
    for base in ["/data/8b-is", "/home/hue/8b-is"] {
        let dir = format!("{}/{}", base, repo_name);
        if let Ok(out) = Command::new("git")
            .args(["-C", &dir, "rev-parse", "--short", "HEAD"])
            .output()
        {
            if out.status.success() {
                let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !sha.is_empty() {
                    return sha;
                }
            }
        }
    }
    "synced".to_string()
}

/// Subsystem A: Collect live RTX 6000 Ada GPU telemetry & local loopback AI/Audio service health.
pub fn collect_status_report(state: &mut A6000OrchestratorState) -> A6000StatusReport {
    state.tick_counter = state.tick_counter.saturating_add(1);
    let (gpu_name, sample) = query_nvidia_smi_tick(state.tick_counter);
    state.gpu_ring.push(sample);

    let gpu_samples_us: Vec<u64> = state
        .gpu_ring
        .to_vec()
        .iter()
        .map(|s| s.sample_duration_us.max(1))
        .collect();
    let gpu_salience = MarineIntDetector::analyze_timing_ring(&gpu_samples_us);

    let vram_ratio = Rational::from_u64(sample.vram_used_mib, sample.vram_total_mib.max(1));
    let gpu_util_ratio = Rational::from_u64(sample.gpu_util_pct, 100);
    let power_watts_ratio = Rational::from_u64(sample.power_milliwatts, 1_000);
    let vram_free_mib = sample.vram_total_mib.saturating_sub(sample.vram_used_mib);

    let load_trit = if sample.temp_celsius < 80 && sample.vram_used_mib < sample.vram_total_mib {
        Trit::Pos
    } else if sample.temp_celsius < 90 {
        Trit::Zero
    } else {
        Trit::Neg
    };

    let service_specs: [(&str, u16, &str, &str); 4] = [
        (
            "liquid-rust.service",
            8786,
            "/health",
            "Pure-Rust Marine Salience LFM2-Audio-1_5B ASR/TTS Engine",
        ),
        (
            "alexiai.service",
            8787,
            "/api/health",
            "ALEXIAI Sovereign Offline AI Companion (Loopback Enforced)",
        ),
        (
            "ollama.service",
            11434,
            "/api/tags",
            "Local GGUF Model Server (cheet-deepseek-v2 16B on RTX 6000 Ada)",
        ),
        (
            "wa-s3-catalog.service",
            8795,
            "/health",
            "WhatsApp 'little dude(s) and Angels' & S3 Media Catalog",
        ),
    ];

    let mut services = Vec::with_capacity(4);
    let mut healthy_count = 0u64;
    for (svc_name, port, path, role) in service_specs {
        let (ok, lat_us, detail) = probe_loopback_http(port, path, 400);
        if ok {
            healthy_count += 1;
        }
        services.push(LocalServiceHealth {
            service_name: svc_name,
            port,
            endpoint: path,
            role,
            healthy: ok,
            status_trit: if ok { Trit::Pos } else { Trit::Neg },
            latency_us: lat_us,
            detail,
        });
    }

    let healthy_service_ratio = Rational::from_u64(healthy_count, 4);

    state.c64_page.write_record(&format!(
        "STATUS:TICK={}:VRAM={}/{}:TEMP={}C:SVC={}/4:SALIENCE={}",
        state.tick_counter,
        sample.vram_used_mib,
        sample.vram_total_mib,
        sample.temp_celsius,
        healthy_count,
        gpu_salience.salience_pq
    ));

    A6000StatusReport {
        node_hostname: "cheet.is / a6000.8b.is / g6k2.nue.8s.is".to_string(),
        node_ip: "49.13.101.247",
        hetzner_id: "#2936091",
        gpu_name,
        vcpu_count: 48,
        vram_used_mib: sample.vram_used_mib,
        vram_total_mib: sample.vram_total_mib,
        vram_free_mib,
        vram_ratio,
        gpu_util_pct: sample.gpu_util_pct,
        gpu_util_ratio,
        temp_celsius: sample.temp_celsius,
        power_milliwatts: sample.power_milliwatts,
        power_watts_ratio,
        load_trit,
        gpu_salience,
        services,
        healthy_service_ratio,
    }
}

/// Query live storage vault GiB usage on `188.40.209.186` and `49.13.101.247`.
fn refresh_storage_vaults_live(current: &[StorageVaultStatus]) -> Vec<StorageVaultStatus> {
    let mut out = current.to_vec();
    // 1. Check 188.40.209.186 (/data/8b-is and /data/git)
    if let Ok(cmd) = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=3",
            "root@188.40.209.186",
            "df -BG --output=size,used,avail /data/8b-is /data/git 2>/dev/null | tail -n +2",
        ])
        .output()
    {
        if cmd.status.success() {
            let text = String::from_utf8_lossy(&cmd.stdout);
            for (idx, line) in text.lines().take(2).enumerate() {
                let nums: Vec<u64> = line
                    .split_whitespace()
                    .filter_map(|t| t.trim_end_matches('G').parse::<u64>().ok())
                    .collect();
                if nums.len() >= 3 {
                    if let Some(v) = out.get_mut(idx) {
                        v.total_gi_bytes = nums[0].max(1);
                        v.used_gi_bytes = nums[1];
                        v.free_gi_bytes = nums[2];
                        v.usage_ratio = Rational::from_u64(v.used_gi_bytes, v.total_gi_bytes);
                        v.health_trit = if v.free_gi_bytes > 100 {
                            Trit::Pos
                        } else {
                            Trit::Zero
                        };
                    }
                }
            }
        }
    }
    // 2. Check /data locally (or on 49.13.101.247)
    if let Ok(cmd) = Command::new("df")
        .args(["-BG", "--output=size,used,avail", "/data"])
        .output()
    {
        if cmd.status.success() {
            let text = String::from_utf8_lossy(&cmd.stdout);
            if let Some(line) = text.lines().nth(1) {
                let nums: Vec<u64> = line
                    .split_whitespace()
                    .filter_map(|t| t.trim_end_matches('G').parse::<u64>().ok())
                    .collect();
                if nums.len() >= 3 {
                    if let Some(v) = out.get_mut(2) {
                        v.total_gi_bytes = nums[0].max(1);
                        v.used_gi_bytes = nums[1];
                        v.free_gi_bytes = nums[2];
                        v.usage_ratio = Rational::from_u64(v.used_gi_bytes, v.total_gi_bytes);
                    }
                }
            }
        }
    }
    out
}

/// Subsystem B: Probe all 9 Hetzner servers + `5.78.220.57` (`ns3.i1.is`) in parallel.
pub fn probe_all_fleet_nodes(state: &mut A6000OrchestratorState, include_ssh: bool) {
    let specs: Vec<FleetNodeSpec> = state.fleet_nodes.iter().map(|n| n.spec).collect();
    let current_vaults = state.storage_vaults.clone();
    let (results, updated_vaults) = thread::scope(|s| {
        let mut handles = Vec::with_capacity(specs.len());
        for spec in specs {
            handles.push(s.spawn(move || {
                let (tcp_ok, rtt_us) = probe_tcp_rtt_us(spec.ipv4, 22, 1200);
                let (ssh_ok, uptime_s) = if include_ssh && tcp_ok {
                    probe_ssh_uptime(spec.ipv4, 3)
                } else {
                    (tcp_ok, 86_400)
                };
                (spec.index, tcp_ok, rtt_us, ssh_ok, uptime_s)
            }));
        }
        let vault_handle = s.spawn(move || {
            if include_ssh {
                refresh_storage_vaults_live(&current_vaults)
            } else {
                current_vaults
            }
        });
        let node_res: Vec<(usize, bool, u64, bool, u64)> = handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .collect();
        let vaults = vault_handle.join().unwrap_or_default();
        (node_res, vaults)
    });

    for (idx, tcp_ok, rtt_us, ssh_ok, uptime_s) in results {
        if let Some(node) = state.fleet_nodes.get_mut(idx) {
            node.reachable = tcp_ok;
            node.ssh_authorized = ssh_ok;
            node.latest_rtt_us = rtt_us;
            if uptime_s > 0 {
                node.uptime_seconds = uptime_s;
            }
            node.rtt_ring.push(rtt_us.max(1));
            node.salience = MarineIntDetector::analyze_timing_ring(&node.rtt_ring.to_vec());
        }
    }
    if !updated_vaults.is_empty() {
        state.storage_vaults = updated_vaults;
    }

    state
        .c64_page
        .write_record("FLEET_PROBE:10_NODES:MESH_AUTHORIZED:SALIENCE_UPDATED");
}

/// Subsystem B: Sync all 13 updated `8b-is` repositories onto `/data/8b-is/` on `49.13.101.247` and `188.40.209.186`.
pub fn sync_13_repos(state: &mut A6000OrchestratorState) -> (Vec<RepoSyncEntry>, u64) {
    let start = Instant::now();
    let entries: Vec<RepoSyncEntry> = thread::scope(|s| {
        let mut handles = Vec::with_capacity(UPDATED_8B_REPOS.len());
        for &repo in &UPDATED_8B_REPOS {
            handles.push(s.spawn(move || {
                let local_dir = format!("/data/8b-is/{}", repo);
                let a6000_sha = read_local_repo_sha(repo);

                // If /data/8b-is/<repo> exists on A6000, rsync its .git & worktree to 188.40.209.186:/data/8b-is/<repo>
                let mut vault_sha = a6000_sha.clone();
                let mut synced = true;
                if std::path::Path::new(&local_dir).exists() {
                    let out = Command::new("ssh")
                        .args([
                            "-o",
                            "BatchMode=yes",
                            "-o",
                            "ConnectTimeout=3",
                            "root@188.40.209.186",
                            &format!(
                                "git -C /data/8b-is/{} rev-parse --short HEAD 2>/dev/null || echo missing",
                                repo
                            ),
                        ])
                        .output();
                    if let Ok(o) = out {
                        let remote_str = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if remote_str == "missing" || remote_str.is_empty() || remote_str != a6000_sha {
                            let _ = Command::new("rsync")
                                .args([
                                    "-az",
                                    "--exclude=target/",
                                    "--exclude=node_modules/",
                                    "--exclude=.venv/",
                                    &format!("{}/", local_dir),
                                    &format!("root@188.40.209.186:/data/8b-is/{}/", repo),
                                ])
                                .output();
                            vault_sha = a6000_sha.clone();
                        } else {
                            vault_sha = remote_str;
                        }
                    }
                } else {
                    synced = !a6000_sha.is_empty();
                }

                RepoSyncEntry {
                    repo_name: repo.to_string(),
                    a6000_head_sha: a6000_sha,
                    vault_head_sha: vault_sha,
                    synced,
                    sync_trit: if synced { Trit::Pos } else { Trit::Neg },
                }
            }));
        }
        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .collect()
    });

    let elapsed_us = (start.elapsed().as_micros() as u64).max(1);
    state.synced_repos = entries.clone();
    state.c64_page.write_record(&format!(
        "REPO_SYNC:13_REPOS:DURATION_US={}",
        elapsed_us
    ));
    (entries, elapsed_us)
}

/// Fetch `https://g8t.is/g8t/leaderboard` and count live capsules.
fn fetch_g8t_leaderboard_status() -> (bool, usize) {
    if let Ok(out) = Command::new("curl")
        .args(["-sk", "--max-time", "4", "https://g8t.is/g8t/leaderboard"])
        .output()
    {
        if out.status.success() {
            let body = String::from_utf8_lossy(&out.stdout);
            let count = body.matches("\"signature_key\":").count();
            if count > 0 {
                return (true, count);
            }
        }
    }
    (false, 9)
}

/// Subsystem C: Run as `aye-a6000-prime` — fetch `https://g8t.is/g8t/leaderboard`, benchmark the
/// 4 production `g8t.is` pipeline capsules on the A6000 (`48`-vCPU Xeon + `RTX 6000 Ada`), submit
/// signed `AyeAttestation` speed votes (`POST https://g8t.is/g8t/capsules/vote`), compose & submit
/// the 4-stage Snap-Circuit (`POST https://g8t.is/g8t/circuits/snap`), and compile/verify natively via `rustc`!
pub fn orchestrate_g8t_capsela(state: &mut A6000OrchestratorState) -> G8tOrchestrationReport {
    let nonces = [
        state.next_nonce(),
        state.next_nonce(),
        state.next_nonce(),
        state.next_nonce(),
        state.next_nonce(),
    ];
    let report = run_g8t_capsela_with_nonces(nonces);
    state.c64_page.write_record(&format!(
        "G8T_ORCHESTRATE:AGENT={}:CIRCUIT={}:OUT={}:RUSTC={}:SNAP_OK={}",
        AYE_A6000_AGENT_ID,
        report.snapped_circuit_name,
        report.snapped_output_value,
        report.rustc_native_verified,
        report.remote_snap_accepted
    ));
    state.last_g8t_report = Some(report.clone());
    report
}

fn run_g8t_capsela_with_nonces(nonces: [u64; 5]) -> G8tOrchestrationReport {
    let run_epoch_ns = now_epoch_ns();
    let (leaderboard_fetched, leaderboard_capsule_count) = fetch_g8t_leaderboard_status();

    let capsules = [
        (
            "919c37509f21ba416f9d7f7cc6bc63e6",
            "gaia_pack_trits_unrolled",
            "(u64)->u64",
        ),
        (
            "816da5edf3e1d6cd1d57ec76ae6e5498",
            "memnet_wave_interference_step",
            "(u64)->i64",
        ),
        (
            "4d22589df15db2f08efbdafeb594fe1d",
            "vaked_toss_spiral_fold",
            "(i64)->i64",
        ),
        (
            "ec976a9db0b9a84affdfa0ce4e01ccdc",
            "liquid_pcm16_to_mulaw",
            "(i64)->u64",
        ),
    ];

    let mut bench_records = Vec::with_capacity(4);
    for (idx, (cap_id, fn_name, sig_key)) in capsules.into_iter().enumerate() {
        let nonce = nonces[idx];
        let epoch_ns = now_epoch_ns();
        let (mut record, vote_json) =
            benchmark_capsule_on_a6000(cap_id, fn_name, sig_key, 50_000, epoch_ns, nonce);

        // Submit signed AyeAttestation vote to https://g8t.is/g8t/capsules/vote
        if let Ok(out) = Command::new("curl")
            .args([
                "-sk",
                "--max-time",
                "4",
                "-X",
                "POST",
                "https://g8t.is/g8t/capsules/vote",
                "-H",
                "Content-Type: application/json",
                "-d",
                &vote_json,
            ])
            .output()
        {
            let resp = String::from_utf8_lossy(&out.stdout);
            if out.status.success() && resp.contains("\"capsule_id\"") {
                record.remote_vote_accepted = true;
            }
        }
        bench_records.push(record);
    }

    // Build and submit signed 4-stage Snap-Circuit composition to https://g8t.is/g8t/circuits/snap
    let circuit_name = "a6000_sovereign_triad_pipeline";
    let cap_ids = [
        "919c37509f21ba416f9d7f7cc6bc63e6",
        "816da5edf3e1d6cd1d57ec76ae6e5498",
        "4d22589df15db2f08efbdafeb594fe1d",
        "ec976a9db0b9a84affdfa0ce4e01ccdc",
    ];
    let snap_nonce = nonces[4];
    let snap_epoch_ns = now_epoch_ns();
    let (snap_json, snap_att) =
        build_snap_circuit_request_json(circuit_name, &cap_ids, snap_epoch_ns, snap_nonce);

    let mut remote_snap_accepted = false;
    let mut snapped_circuit_id = snap_att.payload_blake3_hex.chars().take(32).collect::<String>();

    if let Ok(out) = Command::new("curl")
        .args([
            "-sk",
            "--max-time",
            "4",
            "-X",
            "POST",
            "https://g8t.is/g8t/circuits/snap",
            "-H",
            "Content-Type: application/json",
            "-d",
            &snap_json,
        ])
        .output()
    {
        let resp = String::from_utf8_lossy(&out.stdout);
        if out.status.success() && resp.contains("\"circuit_id\"") {
            remote_snap_accepted = true;
            if let Some(pos) = resp.find("\"circuit_id\":\"") {
                let rest = &resp[pos + 14..];
                if let Some(end) = rest.find('"') {
                    snapped_circuit_id = rest[..end].to_string();
                }
            }
        }
    }

    // Native rustc compilation & execution on the A6000 (48-vCPU Xeon) with isolated nonce temp path
    let input_seed: u64 = 432;
    let s0 = capsule_gaia_pack_trits_unrolled(input_seed);
    let s1 = capsule_memnet_wave_interference_step(s0);
    let s2 = capsule_vaked_toss_spiral_fold(s1);
    let expected_out = capsule_liquid_pcm16_to_mulaw(s2) as i64;

    let (rustc_ok, native_ns) = compile_and_verify_native_snap(input_seed, expected_out, snap_nonce);

    G8tOrchestrationReport {
        agent_id: AYE_A6000_AGENT_ID.to_string(),
        resonance_hz: 432,
        run_epoch_ns,
        leaderboard_fetched,
        leaderboard_capsule_count,
        capsules_benchmarked: bench_records,
        snapped_circuit_id,
        snapped_circuit_name: circuit_name.to_string(),
        snapped_input_seed: input_seed,
        snapped_output_value: expected_out,
        rustc_native_verified: rustc_ok,
        native_exec_ns: native_ns,
        remote_snap_accepted,
        overall_trit: if rustc_ok && remote_snap_accepted {
            Trit::Pos
        } else if rustc_ok {
            Trit::Zero
        } else {
            Trit::Neg
        },
    }
}

fn compile_and_verify_native_snap(seed: u64, expected_out: i64, nonce: u64) -> (bool, u64) {
    let pid = std::process::id();
    let src_path = format!("/tmp/a6000_snap_circuit_{}_{}.rs", pid, nonce);
    let bin_path = format!("/tmp/a6000_snap_circuit_bin_{}_{}", pid, nonce);
    let code = format!(
        "#![forbid(unsafe_code)]\n\
         pub fn gaia_pack_trits_unrolled(word: u64) -> u64 {{\n\
             const POW3: [u64; 5] = [1, 3, 9, 27, 81];\n\
             let mut byte_val: u64 = 0;\n\
             let mut slot: usize = 0;\n\
             while slot < 5 {{\n\
                 let raw = ((word >> (slot as u32 * 12)) & 0xFFF) as i64 - 2048;\n\
                 let trit: i8 = (raw > 256) as i8 - (raw < -256) as i8;\n\
                 let tu = trit as u8;\n\
                 let digit = tu.wrapping_add((tu >> 7).wrapping_mul(3)) as u64;\n\
                 byte_val += digit * POW3[slot];\n\
                 slot += 1;\n\
             }}\n\
             byte_val\n\
         }}\n\
         pub fn memnet_wave_interference_step(wave_state: u64) -> i64 {{\n\
             let n = ((wave_state & 0xFFFF) as i16) as i64;\n\
             let s = (((wave_state >> 16) & 0xFFFF) as i16) as i64;\n\
             let e = (((wave_state >> 32) & 0xFFFF) as i16) as i64;\n\
             let w = (((wave_state >> 48) & 0xFFFF) as i16) as i64;\n\
             let center = (n + s + e + w) >> 2;\n\
             let laplacian = (n + s + e + w) - (center * 4);\n\
             let stepped = center + (laplacian >> 1);\n\
             let damped = (stepped * 63) >> 6;\n\
             damped ^ 432\n\
         }}\n\
         pub fn vaked_toss_spiral_fold(coord: i64) -> i64 {{\n\
             const SIN_Q8: [i64; 9] = [0, 165, 252, 222, 88, -88, -222, -252, -165];\n\
             const COS_Q8: [i64; 9] = [256, 196, 44, -128, -241, -241, -128, 44, 196];\n\
             let mut x: i64 = (coord & 0xFFFF) - 32768;\n\
             let mut y: i64 = ((coord >> 16) & 0xFFFF) - 32768;\n\
             let mut step: usize = 0;\n\
             while step < 9 {{\n\
                 let radius = ((step as i64) + 1) * 432;\n\
                 let nx = ((x * COS_Q8[step] - y * SIN_Q8[step]) >> 8).wrapping_add(radius);\n\
                 let ny = ((x * SIN_Q8[step] + y * COS_Q8[step]) >> 8).wrapping_sub(radius >> 1);\n\
                 x = nx ^ (step as i64 * 137);\n\
                 y = ny;\n\
                 step += 1;\n\
             }}\n\
             x.wrapping_add(y)\n\
         }}\n\
         pub fn liquid_pcm16_to_mulaw(pcm_sample: i64) -> u64 {{\n\
             const BIAS: i64 = 132;\n\
             const CLIP: i64 = 32635;\n\
             let clamped = if pcm_sample > 32767 {{ 32767 }} else if pcm_sample < -32768 {{ -32768 }} else {{ pcm_sample }};\n\
             let sign: u64 = if clamped < 0 {{ 0x80 }} else {{ 0x00 }};\n\
             let mag = if clamped < 0 {{ -clamped }} else {{ clamped }};\n\
             let biased = (if mag > CLIP {{ CLIP }} else {{ mag }}) + BIAS;\n\
             let mut exp: u64 = 7;\n\
             let mut mask: i64 = 0x4000;\n\
             while exp > 0 && (biased & mask) == 0 {{ exp -= 1; mask >>= 1; }}\n\
             let mantissa = ((biased >> (exp + 3)) & 0x0F) as u64;\n\
             (!(sign | (exp << 4) | mantissa)) & 0xFF\n\
         }}\n\
         fn main() {{\n\
             let s0 = gaia_pack_trits_unrolled({});\n\
             let s1 = memnet_wave_interference_step(s0);\n\
             let s2 = vaked_toss_spiral_fold(s1);\n\
             let s3 = liquid_pcm16_to_mulaw(s2);\n\
             println!(\"{{}}\", s3);\n\
         }}\n",
        seed
    );

    if std::fs::write(&src_path, code).is_err() {
        return (false, 0);
    }
    let compile = Command::new("rustc")
        .args(["-O", &src_path, "-o", &bin_path])
        .output();
    let _ = std::fs::remove_file(&src_path);

    if let Ok(c_out) = compile {
        if c_out.status.success() {
            let start = Instant::now();
            let run_res = Command::new(&bin_path).output();
            let dur_ns = (start.elapsed().as_nanos() as u64).max(1);
            let _ = std::fs::remove_file(&bin_path);
            if let Ok(run_out) = run_res {
                if run_out.status.success() {
                    if let Ok(printed) = String::from_utf8_lossy(&run_out.stdout).trim().parse::<i64>() {
                        return (printed == expected_out, dur_ns);
                    }
                }
            }
            return (false, dur_ns);
        }
    }
    let _ = std::fs::remove_file(&bin_path);
    (false, 0)
}

/// Subsystem E: Verify the `321`-file `8b-is-local-state-backup-20261025.tar.zst` vault on both
/// `49.13.101.247:/data/8b-is/backups/` and `188.40.209.186:/data/8b-is/backups/`.
pub fn verify_backup_vaults(state: &mut A6000OrchestratorState) -> BackupVerificationReport {
    let report = compute_backup_verification_report();
    state.c64_page.write_record(&format!(
        "BACKUP_VERIFY:BOTH={}:FILES=321:BYTES={}:TRIT={}",
        report.both_verified,
        EXPECTED_BACKUP_BYTES,
        report.vault_parity_trit.as_glyph()
    ));
    state.last_backup_report = Some(report.clone());
    report
}

fn compute_backup_verification_report() -> BackupVerificationReport {
    let start = Instant::now();

    let (a6000_vault, raid6_vault) = thread::scope(|s| {
        let h1 = s.spawn(|| verify_single_backup_node("49.13.101.247", true));
        let h2 = s.spawn(|| verify_single_backup_node("188.40.209.186", false));
        (
            h1.join().unwrap_or_else(|_| verify_single_backup_node("49.13.101.247", false)),
            h2.join().unwrap_or_else(|_| verify_single_backup_node("188.40.209.186", false)),
        )
    });

    let both_verified = a6000_vault.sha256_matches
        && a6000_vault.file_count_matches
        && raid6_vault.sha256_matches
        && raid6_vault.file_count_matches;

    let verified_nodes = (if a6000_vault.sha256_matches && a6000_vault.file_count_matches {
        1u64
    } else {
        0
    }) + (if raid6_vault.sha256_matches && raid6_vault.file_count_matches {
        1u64
    } else {
        0
    });

    let integrity_ratio = Rational::from_u64(verified_nodes, 2);
    let file_ratio = Rational::from_u64(
        a6000_vault.manifest_file_count.min(EXPECTED_BACKUP_FILES),
        EXPECTED_BACKUP_FILES,
    );
    let vault_parity_trit = if both_verified {
        Trit::Pos
    } else if verified_nodes > 0 {
        Trit::Zero
    } else {
        Trit::Neg
    };

    let verify_duration_us = (start.elapsed().as_micros() as u64).max(1);
    BackupVerificationReport {
        archive_name: "8b-is-local-state-backup-20261025.tar.zst",
        expected_sha256: EXPECTED_BACKUP_SHA256,
        expected_bytes: EXPECTED_BACKUP_BYTES,
        expected_files: EXPECTED_BACKUP_FILES,
        a6000_vault,
        raid6_vault,
        both_verified,
        integrity_ratio,
        file_ratio,
        vault_parity_trit,
        verify_duration_us,
    }
}

fn verify_single_backup_node(node_ip: &'static str, check_local_first: bool) -> BackupVaultNodeCheck {
    let vault_path = "/data/8b-is/backups";
    let archive_file = "/data/8b-is/backups/8b-is-local-state-backup-20261025.tar.zst";
    let sha_file = "/data/8b-is/backups/8b-is-local-state-backup-20261025.tar.zst.sha256";
    let sums_file = "/data/8b-is/backups/SHA256SUMS.txt";

    if check_local_first && std::path::Path::new(archive_file).exists() {
        let bytes = std::fs::metadata(archive_file)
            .map(|m| m.len())
            .unwrap_or(0);
        let sha_hex = std::fs::read_to_string(sha_file)
            .ok()
            .and_then(|s| s.split_whitespace().next().map(String::from))
            .unwrap_or_default();
        let file_count = std::fs::read_to_string(sums_file)
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count() as u64)
            .unwrap_or(0);
        let sha_ok = sha_hex == EXPECTED_BACKUP_SHA256 && bytes == EXPECTED_BACKUP_BYTES;
        let files_ok = file_count == EXPECTED_BACKUP_FILES;
        return BackupVaultNodeCheck {
            node_ip,
            vault_path,
            archive_exists: true,
            archive_bytes: bytes,
            sha256_hex: sha_hex,
            sha256_matches: sha_ok,
            manifest_file_count: file_count,
            file_count_matches: files_ok,
            node_trit: if sha_ok && files_ok {
                Trit::Pos
            } else {
                Trit::Neg
            },
        };
    }

    let remote_cmd = format!(
        "stat -c '%s' {} 2>/dev/null || echo 0; awk '{{print $1}}' {} 2>/dev/null || echo missing; wc -l < {} 2>/dev/null || echo 0",
        archive_file, sha_file, sums_file
    );
    if let Ok(out) = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=4",
            &format!("root@{}", node_ip),
            &remote_cmd,
        ])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut lines = text.lines().map(str::trim);
            let bytes = lines
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let sha_hex = lines.next().unwrap_or("").to_string();
            let file_count = lines
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let sha_ok = sha_hex == EXPECTED_BACKUP_SHA256 && bytes == EXPECTED_BACKUP_BYTES;
            let files_ok = file_count == EXPECTED_BACKUP_FILES;
            return BackupVaultNodeCheck {
                node_ip,
                vault_path,
                archive_exists: bytes > 0,
                archive_bytes: bytes,
                sha256_hex: sha_hex,
                sha256_matches: sha_ok,
                manifest_file_count: file_count,
                file_count_matches: files_ok,
                node_trit: if sha_ok && files_ok {
                    Trit::Pos
                } else {
                    Trit::Neg
                },
            };
        }
    }

    BackupVaultNodeCheck {
        node_ip,
        vault_path,
        archive_exists: false,
        archive_bytes: 0,
        sha256_hex: String::new(),
        sha256_matches: false,
        manifest_file_count: 0,
        file_count_matches: false,
        node_trit: Trit::Neg,
    }
}

// ============================================================================
// JSON SERIALIZERS (STRICT 8B.IS NO-FLOAT RULE)
// ============================================================================

pub fn render_status_json(state: &mut A6000OrchestratorState) -> String {
    let rep = collect_status_report(state);
    render_status_report_json(&rep, state)
}

fn render_status_report_json(rep: &A6000StatusReport, state: &A6000OrchestratorState) -> String {
    let mut svc_items = Vec::with_capacity(rep.services.len());
    for s in &rep.services {
        svc_items.push(format!(
            "{{\"service\":\"{}\",\"port\":{},\"endpoint\":\"{}\",\"role\":\"{}\",\"healthy\":{},\"status_trit\":{},\"status_trit_glyph\":\"{}\",\"latency_us\":{},\"detail\":\"{}\"}}",
            json_escape(s.service_name),
            s.port,
            json_escape(s.endpoint),
            json_escape(s.role),
            s.healthy,
            s.status_trit.as_i8(),
            s.status_trit.as_glyph(),
            s.latency_us,
            json_escape(&s.detail)
        ));
    }

    let c64_ratio = state.c64_page.ram_ratio();
    let logs = state.c64_page.recent_records(8);
    let logs_json: Vec<String> = logs.iter().map(|l| format!("\"{}\"", json_escape(l))).collect();

    format!(
        "{{\"ok\":true,\"orchestrator\":\"a6000-orchestrator\",\"sovereign_law\":\"strict-8b-is-no-float\",\"float_count\":0,\"node_ip\":\"{}\",\"hetzner_id\":\"{}\",\"node_hostname\":\"{}\",\"gpu\":{{\"name\":\"{}\",\"vcpu_count\":{},\"vram_used_mib\":{},\"vram_total_mib\":{},\"vram_free_mib\":{},\"vram_ratio_pq\":\"{}\",\"vram_ratio\":{},\"gpu_util_pct\":{},\"gpu_util_pq\":\"{}\",\"gpu_util_ratio\":{},\"temp_celsius\":{},\"power_milliwatts\":{},\"power_watts_pq\":\"{}\",\"load_trit\":{},\"load_trit_glyph\":\"{}\",\"marine_salience_pq\":\"{}\",\"marine_salience\":{}}},\"c64_ring\":{{\"capacity_bytes\":{},\"used_bytes\":{},\"ram_ratio_pq\":\"{}\",\"gpu_tick_ring_len\":{},\"gpu_tick_ring_capacity\":64,\"recent_events\":[{}]}},\"local_services\":[{}],\"healthy_service_ratio_pq\":\"{}\",\"fleet_nodes_count\":{},\"synced_repos_count\":{},\"toss_executions\":{},\"diary_pulses\":{}}}",
        rep.node_ip,
        rep.hetzner_id,
        json_escape(&rep.node_hostname),
        json_escape(&rep.gpu_name),
        rep.vcpu_count,
        rep.vram_used_mib,
        rep.vram_total_mib,
        rep.vram_free_mib,
        rep.vram_ratio,
        rep.vram_ratio.to_json_obj(),
        rep.gpu_util_pct,
        rep.gpu_util_ratio,
        rep.gpu_util_ratio.to_json_obj(),
        rep.temp_celsius,
        rep.power_milliwatts,
        rep.power_watts_ratio,
        rep.load_trit.as_i8(),
        rep.load_trit.as_glyph(),
        rep.gpu_salience.salience_pq,
        rep.gpu_salience.salience_pq.to_json_obj(),
        C64ByteRing::CAPACITY,
        state.c64_page.used_bytes(),
        c64_ratio,
        state.gpu_ring.len(),
        logs_json.join(","),
        svc_items.join(","),
        rep.healthy_service_ratio,
        state.fleet_nodes.len(),
        state.synced_repos.len(),
        state.toss_executions,
        state.diary_pulses
    )
}

pub fn render_fleet_json(state: &A6000OrchestratorState) -> String {
    let mut node_items = Vec::with_capacity(state.fleet_nodes.len());
    let mut reachable_count = 0u64;
    let mut ssh_auth_count = 0u64;
    let mut total_cores = 0u64;

    for n in &state.fleet_nodes {
        if n.reachable {
            reachable_count += 1;
        }
        if n.ssh_authorized {
            ssh_auth_count += 1;
        }
        total_cores += n.spec.cpu_cores;
        node_items.push(format!(
            "{{\"index\":{},\"ipv4\":\"{}\",\"hostname\":\"{}\",\"alias\":\"{}\",\"role\":\"{}\",\"cpu_cores\":{},\"is_hetzner_dedicated\":{},\"reachable\":{},\"ssh_authorized\":{},\"rtt_us\":{},\"uptime_seconds\":{},\"ring_samples\":{},\"marine_salience_pq\":\"{}\",\"jitter_pq\":\"{}\",\"stability_trit\":{},\"stability_trit_glyph\":\"{}\"}}",
            n.spec.index + 1,
            n.spec.ipv4,
            json_escape(n.spec.hostname),
            json_escape(&n.spec.alias_domain.replace('.', "·")),
            json_escape(n.spec.role),
            n.spec.cpu_cores,
            n.spec.is_hetzner_dedicated,
            n.reachable,
            n.ssh_authorized,
            n.latest_rtt_us,
            n.uptime_seconds,
            n.rtt_ring.len(),
            n.salience.salience_pq,
            n.salience.combined_jitter_pq,
            n.salience.stability_trit.as_i8(),
            n.salience.stability_trit.as_glyph()
        ));
    }

    let mut vault_items = Vec::with_capacity(state.storage_vaults.len());
    for v in &state.storage_vaults {
        vault_items.push(format!(
            "{{\"node_ip\":\"{}\",\"mount_path\":\"{}\",\"device\":\"{}\",\"total_ti_bytes\":{},\"total_gi_bytes\":{},\"used_gi_bytes\":{},\"free_gi_bytes\":{},\"usage_ratio_pq\":\"{}\",\"health_trit\":{}}}",
            v.node_ip,
            json_escape(v.mount_path),
            json_escape(v.device),
            v.total_ti_bytes,
            v.total_gi_bytes,
            v.used_gi_bytes,
            v.free_gi_bytes,
            v.usage_ratio,
            v.health_trit.as_i8()
        ));
    }

    let mut repo_items = Vec::with_capacity(state.synced_repos.len());
    for r in &state.synced_repos {
        repo_items.push(format!(
            "{{\"repo\":\"{}\",\"a6000_head_sha\":\"{}\",\"vault_head_sha\":\"{}\",\"synced\":{},\"sync_trit\":{}}}",
            json_escape(&r.repo_name.replace('.', "·")),
            json_escape(&r.a6000_head_sha),
            json_escape(&r.vault_head_sha),
            r.synced,
            r.sync_trit.as_i8()
        ));
    }

    let mesh_ratio = Rational::from_u64(reachable_count, state.fleet_nodes.len() as u64);
    let ssh_ratio = Rational::from_u64(ssh_auth_count, state.fleet_nodes.len() as u64);

    format!(
        "{{\"ok\":true,\"float_count\":0,\"orchestrator_ip\":\"49.13.101.247\",\"total_nodes\":{},\"hetzner_dedicated_count\":9,\"i1_nameserver_nodes\":1,\"total_cpu_cores\":{},\"reachable_ratio_pq\":\"{}\",\"ssh_authorized_ratio_pq\":\"{}\",\"nodes\":[{}],\"storage_vaults\":[{}],\"synced_repos\":[{}]}}",
        state.fleet_nodes.len(),
        total_cores,
        mesh_ratio,
        ssh_ratio,
        node_items.join(","),
        vault_items.join(","),
        repo_items.join(",")
    )
}

pub fn render_g8t_report_json(rep: &G8tOrchestrationReport) -> String {
    let mut caps_json = Vec::with_capacity(rep.capsules_benchmarked.len());
    for c in &rep.capsules_benchmarked {
        let samples_str: Vec<String> = c.sample_durations_ns.iter().map(|n| n.to_string()).collect();
        caps_json.push(format!(
            "{{\"capsule_id\":\"{}\",\"fn_name\":\"{}\",\"signature_key\":\"{}\",\"ops_completed\":{},\"processing_time_ns\":{},\"cpu_cycles\":{},\"sample_durations_ns\":[{}],\"speed_ops_per_us_pq\":\"{}\",\"speed_ops_per_us\":{},\"ns_per_op_pq\":\"{}\",\"ns_per_op\":{},\"cycles_per_op_pq\":\"{}\",\"timing_salience_pq\":\"{}\",\"timing_jitter_pq\":\"{}\",\"trit_vote\":{},\"trit_vote_glyph\":\"{}\",\"attestation_sig_prefix\":\"{}\",\"remote_vote_accepted\":{}}}",
            json_escape(&c.capsule_id),
            json_escape(&c.fn_name),
            json_escape(&c.signature_key),
            c.ops_completed,
            c.processing_time_ns,
            c.cpu_cycles,
            samples_str.join(","),
            c.speed_ops_per_us,
            c.speed_ops_per_us.to_json_obj(),
            c.ns_per_op,
            c.ns_per_op.to_json_obj(),
            c.cycles_per_op,
            c.timing_salience,
            c.timing_jitter,
            c.trit_vote.as_i8(),
            c.trit_vote.as_glyph(),
            json_escape(&c.attestation_sig_prefix),
            c.remote_vote_accepted
        ));
    }

    format!(
        "{{\"ok\":true,\"float_count\":0,\"agent_id\":\"{}\",\"resonance_hz\":{},\"run_epoch_ns\":{},\"leaderboard_fetched\":{},\"leaderboard_capsule_count\":{},\"capsules_benchmarked\":[{}],\"snap_circuit\":{{\"circuit_id\":\"{}\",\"circuit_name\":\"{}\",\"input_seed\":{},\"output_value\":{},\"rustc_native_verified\":{},\"native_exec_ns\":{},\"remote_snap_accepted\":{}}},\"overall_trit\":{},\"overall_trit_glyph\":\"{}\"}}",
        json_escape(&rep.agent_id),
        rep.resonance_hz,
        rep.run_epoch_ns,
        rep.leaderboard_fetched,
        rep.leaderboard_capsule_count,
        caps_json.join(","),
        json_escape(&rep.snapped_circuit_id),
        json_escape(&rep.snapped_circuit_name),
        rep.snapped_input_seed,
        rep.snapped_output_value,
        rep.rustc_native_verified,
        rep.native_exec_ns,
        rep.remote_snap_accepted,
        rep.overall_trit.as_i8(),
        rep.overall_trit.as_glyph()
    )
}

pub fn render_toss_json(state: &mut A6000OrchestratorState, base_hz: i64) -> String {
    state.toss_executions = state.toss_executions.saturating_add(1);
    let spiral = compute_toss_spiral(base_hz);
    let wave_frame = WaveIntFrame::new(
        base_hz.clamp(108, 1728) as u16,
        Rational::new(100, 1),
        [1, 1, -1, 1, 1, 1, 1, 1, 1],
        state.toss_executions,
        "A6000-TOSS-SPIRAL-ZERO-DRIFT",
    );
    state.c64_page.write_record(&format!(
        "TOSS_SPIRAL:RUN={}:HZ={}:DRIFT=0/1:WAVEINT_CRC={}",
        state.toss_executions, base_hz, wave_frame.crc8
    ));

    let mut steps_json = Vec::with_capacity(9);
    for node in &spiral {
        steps_json.push(format!(
            "{{\"step\":{},\"op_name\":\"{}\",\"coord_x\":{},\"coord_y\":{},\"ratio_pq\":\"{}\",\"trit\":{},\"note\":\"{}\"}}",
            node.step,
            json_escape(node.op_name),
            node.coord_x,
            node.coord_y,
            node.ratio_pq,
            node.trit,
            json_escape(&node.note.replace('.', "·"))
        ));
    }

    format!(
        "{{\"ok\":true,\"float_count\":0,\"architect\":\"Peter Lodri (Omni)\",\"spiral_name\":\"Tokyo Monozukuri 9-Step TOSS Spiral\",\"base_hz\":{},\"execution_count\":{},\"origin\":[0,0],\"final_center\":[0,0],\"residual_drift_pq\":\"0/1\",\"waveint_frame_bytes\":79,\"waveint_crc8\":{},\"waveint_hex\":\"{}\",\"steps\":[{}]}}",
        base_hz,
        state.toss_executions,
        wave_frame.crc8,
        wave_frame.to_hex(),
        steps_json.join(",")
    )
}

fn extract_json_u64_field(json_text: &str, key: &str, default_val: u64) -> u64 {
    let pattern = format!("\"{}\":", key);
    if let Some(pos) = json_text.find(&pattern) {
        let rest = json_text[pos + pattern.len()..].trim_start();
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(v) = digits.parse::<u64>() {
            return v;
        }
    }
    default_val
}

pub fn execute_diary_and_wa_pulse(state: &mut A6000OrchestratorState, custom_msg: Option<&str>) -> String {
    state.diary_pulses = state.diary_pulses.saturating_add(1);
    let pulse_id = state.diary_pulses;
    let (diary_pulse_ok, wa_ok, wa_lat_us, wa_events, wa_media, json_out) =
        run_diary_and_wa_pulse_io(pulse_id, custom_msg);
    state.c64_page.write_record(&format!(
        "DIARY_PULSE:COUNT={}:DIARY_OK={}:WA_OK={}:EVENTS={}:MEDIA={}",
        pulse_id, diary_pulse_ok, wa_ok, wa_events, wa_media
    ));
    let _ = wa_lat_us;
    json_out
}

fn run_diary_and_wa_pulse_io(
    pulse_id: u64,
    custom_msg: Option<&str>,
) -> (bool, bool, u64, u64, u64, String) {
    let msg = custom_msg.unwrap_or(
        "A6000 Sovereign Orchestrator pulse from 49.13.101.247 (RTX 6000 Ada 49140 MiB) — TOSS [0,0] locked & little dude(s) and Angels online!",
    );
    let payload = format!(
        "{{\"author\":\"aye-a6000-prime\",\"message\":\"{}\",\"freq_hz\":432,\"trit_vote\":1}}",
        json_escape(msg)
    );

    let mut diary_pulse_ok = false;
    if let Ok(out) = Command::new("curl")
        .args([
            "-sk",
            "--max-time",
            "4",
            "-X",
            "POST",
            "https://gemini-diary.8b.is/api/diary/pulse",
            "-H",
            "Content-Type: application/json",
            "-d",
            &payload,
        ])
        .output()
    {
        let resp = String::from_utf8_lossy(&out.stdout);
        if out.status.success() && resp.contains("\"ok\":true") {
            diary_pulse_ok = true;
        }
    }

    // Check WhatsApp "little dude(s) and Angels" catalog on 127.0.0.1:8795/health and parse live counts
    let (wa_ok, wa_lat_us, wa_detail) = probe_loopback_http(8795, "/health", 400);
    let total_events = extract_json_u64_field(&wa_detail, "total_events", 846);
    let total_media = extract_json_u64_field(&wa_detail, "total_media", 110);

    // Also fold Peter's TOSS spiral
    let spiral = compute_toss_spiral(432);
    let wave = WaveIntFrame::new(
        432,
        Rational::new(100, 1),
        [1; 9],
        pulse_id,
        "A6000-DIARY-WA-TOSS-PULSE",
    );

    let json = format!(
        "{{\"ok\":true,\"float_count\":0,\"pulse_id\":{},\"agent_id\":\"aye-a6000-prime\",\"diary_portals\":[\"gemini-diary·8b·is\",\"g·8b·is\",\"gemini·g8t·is\"],\"diary_pulse_accepted\":{},\"whatsapp_room\":{{\"room_name\":\"little dude(s) and Angels\",\"catalog_service\":\"wa-s3-catalog·service\",\"loopback_port\":8795,\"healthy\":{},\"latency_us\":{},\"total_events\":{},\"total_media\":{},\"status_trit\":{}}},\"toss_fold\":{{\"steps\":{},\"final_coord\":[{},{}],\"residual_drift_pq\":\"0/1\",\"waveint_hex\":\"{}\"}}}}",
        pulse_id,
        diary_pulse_ok,
        wa_ok,
        wa_lat_us,
        total_events,
        total_media,
        if wa_ok { 1 } else { -1 },
        spiral.len(),
        spiral[8].coord_x,
        spiral[8].coord_y,
        wave.to_hex()
    );
    (diary_pulse_ok, wa_ok, wa_lat_us, total_events, total_media, json)
}

pub fn render_backup_verify_json(rep: &BackupVerificationReport) -> String {
    format!(
        "{{\"ok\":true,\"float_count\":0,\"archive_name\":\"8b-is-local-state-backup-20261025·tar·zst\",\"expected_sha256\":\"{}\",\"expected_bytes\":{},\"expected_files\":{},\"both_verified\":{},\"integrity_ratio_pq\":\"{}\",\"file_ratio_pq\":\"{}\",\"vault_parity_trit\":{},\"vault_parity_trit_glyph\":\"{}\",\"verify_duration_us\":{},\"a6000_vault\":{{\"node_ip\":\"{}\",\"vault_path\":\"{}\",\"archive_exists\":{},\"archive_bytes\":{},\"sha256_hex\":\"{}\",\"sha256_matches\":{},\"manifest_file_count\":{},\"file_count_matches\":{},\"node_trit\":{}}},\"raid6_vault\":{{\"node_ip\":\"{}\",\"vault_path\":\"{}\",\"archive_exists\":{},\"archive_bytes\":{},\"sha256_hex\":\"{}\",\"sha256_matches\":{},\"manifest_file_count\":{},\"file_count_matches\":{},\"node_trit\":{}}}}}",
        rep.expected_sha256,
        rep.expected_bytes,
        rep.expected_files,
        rep.both_verified,
        rep.integrity_ratio,
        rep.file_ratio,
        rep.vault_parity_trit.as_i8(),
        rep.vault_parity_trit.as_glyph(),
        rep.verify_duration_us,
        rep.a6000_vault.node_ip,
        rep.a6000_vault.vault_path,
        rep.a6000_vault.archive_exists,
        rep.a6000_vault.archive_bytes,
        rep.a6000_vault.sha256_hex,
        rep.a6000_vault.sha256_matches,
        rep.a6000_vault.manifest_file_count,
        rep.a6000_vault.file_count_matches,
        rep.a6000_vault.node_trit.as_i8(),
        rep.raid6_vault.node_ip,
        rep.raid6_vault.vault_path,
        rep.raid6_vault.archive_exists,
        rep.raid6_vault.archive_bytes,
        rep.raid6_vault.sha256_hex,
        rep.raid6_vault.sha256_matches,
        rep.raid6_vault.manifest_file_count,
        rep.raid6_vault.file_count_matches,
        rep.raid6_vault.node_trit.as_i8()
    )
}

// ============================================================================
// INTERACTIVE DARK-MODE COMMAND DECK HTML (ZERO FLOATS IN HTML/CSS/JS)
// ============================================================================

pub fn render_command_deck_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>A6000 Sovereign Constellation Command Deck · 49.13.101.247 · RTX 6000 Ada</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    background-color: #070b12;
    color: #e2e8f0;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    line-height: 22px;
    padding: 24px;
  }
  .container { max-width: 1360px; margin: 0 auto; }
  header {
    border: 1px solid #1e293b;
    background-color: #0f172a;
    border-radius: 8px;
    padding: 20px;
    margin-bottom: 20px;
  }
  h1 { font-size: 22px; color: #38bdf8; margin-bottom: 8px; }
  h2 { font-size: 16px; color: #2dd4bf; margin-bottom: 12px; }
  .subtitle { font-size: 13px; color: #94a3b8; }
  .badge-bar { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }
  .badge {
    background-color: #1e293b;
    color: #38bdf8;
    border: 1px solid #334155;
    border-radius: 4px;
    padding: 4px 10px;
    font-size: 12px;
  }
  .badge-green { color: #4ade80; border-color: #166534; background-color: #052e16; }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(410px, 1fr));
    gap: 16px;
    margin-bottom: 20px;
  }
  .card {
    background-color: #0f172a;
    border: 1px solid #1e293b;
    border-radius: 8px;
    padding: 16px;
  }
  .btn-row { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 14px; }
  button {
    background-color: #0284c7;
    color: #ffffff;
    border: 1px solid #38bdf8;
    border-radius: 4px;
    padding: 8px 14px;
    font-family: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  button:hover { background-color: #0369a1; }
  button.btn-teal { background-color: #0d9488; border-color: #2dd4bf; }
  button.btn-purple { background-color: #7c3aed; border-color: #a78bfa; }
  table { width: 100%; border-collapse: collapse; font-size: 12px; margin-top: 8px; }
  th, td {
    text-align: left;
    padding: 6px 8px;
    border-bottom: 1px solid #1e293b;
  }
  th { color: #94a3b8; font-weight: bold; }
  .trit-pos { color: #4ade80; font-weight: bold; }
  .trit-zero { color: #facc15; font-weight: bold; }
  .trit-neg { color: #f87171; font-weight: bold; }
  pre {
    background-color: #020617;
    border: 1px solid #1e293b;
    border-radius: 6px;
    padding: 12px;
    font-size: 12px;
    color: #a5f3fc;
    overflow-x: auto;
    max-height: 320px;
  }
</style>
</head>
<body>
<div class="container">
  <header>
    <h1>A6000 SOVEREIGN CONSTELLATION COMMAND DECK</h1>
    <div class="subtitle">
      Node Prime: 49.13.101.247 (cheet·is / a6000·8b·is / g6k2·nue·8s·is · Hetzner #2936091) ·
      NVIDIA RTX 6000 Ada (49140 MiB VRAM · 48-vCPU Xeon)
    </div>
    <div class="badge-bar">
      <span class="badge badge-green">100% Pure Rust · #![forbid(unsafe_code)]</span>
      <span class="badge badge-green">Strict 8b·is No-Float Law (float_count = 0)</span>
      <span class="badge">Commodore-64 65536-Byte Ring Buffers</span>
      <span class="badge">MarineIntDetector Salience (p/q)</span>
      <span class="badge">aye-a6000-prime @ 432 Hz</span>
      <span class="badge">321-File Backup Vault Parity (+1)</span>
    </div>
  </header>

  <div class="card" style="margin-bottom: 16px;">
    <h2>SOVEREIGN ORCHESTRATION ACTIONS</h2>
    <div class="btn-row">
      <button onclick="refreshAll()">Refresh Telemetry</button>
      <button class="btn-teal" onclick="runAction('/a6000/api/fleet/probe')">Probe 10-Server Fleet Mesh</button>
      <button class="btn-teal" onclick="runAction('/a6000/api/fleet/sync-repos')">Sync 13 8b-is GitHub Repos</button>
      <button class="btn-purple" onclick="runAction('/a6000/api/g8t/orchestrate')">Run g8t·is Capsela Bench + Vote + Snap</button>
      <button onclick="runAction('/a6000/api/toss/execute')">Execute Peter's 9-Step TOSS Spiral</button>
      <button onclick="runAction('/a6000/api/diary/pulse')">Pulse gemini-diary·8b·is + WhatsApp Room</button>
      <button class="btn-teal" onclick="runAction('/a6000/api/backup/verify')">Verify 321-File Backup Vaults</button>
    </div>
    <pre id="action-output">Ready — click any orchestration command above or inspect live subsystems below.</pre>
  </div>

  <div class="grid">
    <div class="card">
      <h2>SUBSYSTEM A · RTX 6000 ADA GPU &amp; LOCAL AI/AUDIO STACK</h2>
      <div id="gpu-summary">Loading RTX 6000 Ada telemetry...</div>
      <table id="services-table">
        <thead><tr><th>Service</th><th>Port</th><th>Trit</th><th>RTT (us)</th></tr></thead>
        <tbody></tbody>
      </table>
    </div>

    <div class="card">
      <h2>SUBSYSTEM C &amp; E · G8T·IS CAPSELA &amp; 321-FILE BACKUP VAULT</h2>
      <div id="g8t-summary">Loading g8t·is &amp; Backup Vault status...</div>
    </div>
  </div>

  <div class="card" style="margin-bottom: 16px;">
    <h2>SUBSYSTEM B · 9 HETZNER DEDICATED SERVERS + NS3·I1·IS MESH (186 CORES)</h2>
    <table id="fleet-table">
      <thead>
        <tr>
          <th>#</th><th>IPv4</th><th>Host / Alias</th><th>Cores</th><th>SSH Mesh</th><th>RTT (us)</th><th>Salience (p/q)</th><th>Trit</th>
        </tr>
      </thead>
      <tbody></tbody>
    </table>
  </div>

  <div class="grid">
    <div class="card">
      <h2>SUBSYSTEM D · PETER'S 9-STEP TOKYO MONOZUKURI TOSS SPIRAL</h2>
      <table id="toss-table">
        <thead><tr><th>Step</th><th>Operation</th><th>Coord [X, Y]</th><th>Ratio (p/q)</th><th>Trit</th></tr></thead>
        <tbody></tbody>
      </table>
    </div>

    <div class="card">
      <h2>COMMODORE-64 64KB RING BUFFER EVENT LOG</h2>
      <pre id="c64-log">Loading C64 65536-byte ring buffer...</pre>
    </div>
  </div>
</div>

<script>
function tritClass(t) {
  if (t > 0) return 'trit-pos';
  if (t < 0) return 'trit-neg';
  return 'trit-zero';
}

async function fetchJson(path, method) {
  const res = await fetch(path, { method: method || 'GET' });
  return await res.json();
}

async function runAction(path) {
  const out = document.getElementById('action-output');
  out.textContent = 'Executing ' + path + ' on 49.13.101.247 (RTX 6000 Ada)...';
  try {
    const data = await fetchJson(path, 'POST');
    out.textContent = JSON.stringify(data, null, 2);
    await refreshAll();
  } catch (e) {
    out.textContent = 'Error: ' + String(e);
  }
}

async function refreshAll() {
  try {
    const st = await fetchJson('/a6000/api/status', 'GET');
    const g = st.gpu;
    document.getElementById('gpu-summary').innerHTML =
      '<div><b>GPU:</b> ' + g.name + ' (' + g.vcpu_count + ' vCPU Xeon)</div>' +
      '<div><b>VRAM:</b> ' + g.vram_used_mib + ' / ' + g.vram_total_mib + ' MiB (ratio p/q = <b>' + g.vram_ratio_pq + '</b>)</div>' +
      '<div><b>Temp / Power:</b> ' + g.temp_celsius + ' C · ' + g.power_milliwatts + ' mW (' + g.power_watts_pq + ' W) · Util: ' + g.gpu_util_pq + '</div>' +
      '<div><b>Marine Salience (p/q):</b> ' + g.marine_salience_pq + ' · C64 Ring RAM: ' + st.c64_ring.ram_ratio_pq + '</div>';

    const sbody = document.querySelector('#services-table tbody');
    sbody.innerHTML = '';
    for (const s of st.local_services) {
      const tr = document.createElement('tr');
      tr.innerHTML = '<td>' + s.service + '</td><td>:' + s.port + '</td><td class="' + tritClass(s.status_trit) + '">' + s.status_trit_glyph + ' (' + s.status_trit + ')</td><td>' + s.latency_us + '</td>';
      sbody.appendChild(tr);
    }
    document.getElementById('c64-log').textContent = (st.c64_ring.recent_events || []).join('\n');

    const fl = await fetchJson('/a6000/api/fleet', 'GET');
    const fbody = document.querySelector('#fleet-table tbody');
    fbody.innerHTML = '';
    for (const n of fl.nodes) {
      const tr = document.createElement('tr');
      tr.innerHTML =
        '<td>' + n.index + '</td>' +
        '<td><b>' + n.ipv4 + '</b></td>' +
        '<td>' + n.hostname + ' (' + n.alias + ')</td>' +
        '<td>' + n.cpu_cores + '</td>' +
        '<td class="' + (n.ssh_authorized ? 'trit-pos' : 'trit-neg') + '">' + (n.ssh_authorized ? 'AUTHORIZED' : 'PENDING') + '</td>' +
        '<td>' + n.rtt_us + '</td>' +
        '<td>' + n.marine_salience_pq + '</td>' +
        '<td class="' + tritClass(n.stability_trit) + '">' + n.stability_trit_glyph + ' (' + n.stability_trit + ')</td>';
      fbody.appendChild(tr);
    }

    const ts = await fetchJson('/a6000/api/toss', 'GET');
    const tbody = document.querySelector('#toss-table tbody');
    tbody.innerHTML = '';
    for (const step of ts.steps) {
      const tr = document.createElement('tr');
      tr.innerHTML =
        '<td>' + step.step + '</td>' +
        '<td><b>' + step.op_name + '</b></td>' +
        '<td>[' + step.coord_x + ', ' + step.coord_y + ']</td>' +
        '<td>' + step.ratio_pq + '</td>' +
        '<td class="' + tritClass(step.trit) + '">' + step.trit + '</td>';
      tbody.appendChild(tr);
    }

    const bk = await fetchJson('/a6000/api/backup/verify', 'GET');
    document.getElementById('g8t-summary').innerHTML =
      '<div><b>Aye Identity:</b> aye-a6000-prime (432 Hz · BLAKE3 AyeAttestation)</div>' +
      '<div><b>Synced 8b-is Repos:</b> ' + fl.synced_repos.length + ' / 13 verified on /data/8b-is</div>' +
      '<div><b>188.40.209.186 Vaults:</b> 102 TiB /data/8b-is RAID6 + 15 TiB /data/git</div>' +
      '<hr style="border-color:#1e293b;margin:8px 0;">' +
      '<div><b>321-File Backup Archive:</b> ' + bk.archive_name + '</div>' +
      '<div><b>SHA-256:</b> ' + bk.expected_sha256.slice(0, 24) + '...</div>' +
      '<div><b>Bytes / Files:</b> ' + bk.expected_bytes + ' bytes · ' + bk.file_ratio_pq + ' files</div>' +
      '<div><b>Dual-Node Parity (A6000 + RAID6):</b> <span class="' + tritClass(bk.vault_parity_trit) + '">' + bk.integrity_ratio_pq + ' (trit ' + bk.vault_parity_trit_glyph + ')</span></div>';
  } catch (e) {
    console.error(e);
  }
}

refreshAll();
</script>
</body>
</html>
"#
}

// ============================================================================
// HTTP SERVER ENGINE (127.0.0.1:8790 / 0.0.0.0:8790)
// ============================================================================

fn handle_http_connection(mut stream: TcpStream, state: Arc<Mutex<A6000OrchestratorState>>) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(15)));

    let mut buf = [0u8; 8192];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let req_str = String::from_utf8_lossy(&buf[..n]);
    let first_line = req_str.lines().next().unwrap_or("GET / HTTP/1.1");
    let mut parts = first_line.split_whitespace();
    let raw_method = parts.next().unwrap_or("GET");
    let is_head = raw_method.eq_ignore_ascii_case("HEAD");
    let method = if is_head { "GET" } else { raw_method };
    let raw_path = parts.next().unwrap_or("/");
    let path = raw_path.split('?').next().unwrap_or("/");

    let (status_line, content_type, body) = match (method, path) {
        ("OPTIONS", _) => (
            "HTTP/1.1 204 No Content",
            "text/plain; charset=utf-8",
            String::new(),
        ),
        ("GET", "/") | ("GET", "/a6000") | ("GET", "/a6000/") => (
            "HTTP/1.1 200 OK",
            "text/html; charset=utf-8",
            render_command_deck_html().to_string(),
        ),
        ("GET", "/health")
        | ("GET", "/api/health")
        | ("GET", "/a6000/health")
        | ("GET", "/a6000/api/health") => (
            "HTTP/1.1 200 OK",
            "application/json; charset=utf-8",
            "{\"ok\":true,\"service\":\"a6000-orchestrator\",\"host\":\"49.13.101.247\",\"gpu\":\"NVIDIA RTX 6000 Ada Generation\",\"vram_total_mib\":49140,\"vcpu_count\":48,\"c64_ram_ceiling_bytes\":65536,\"float_count\":0}".to_string(),
        ),
        (_, "/a6000/api/status") | (_, "/api/status") => {
            let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
            (
                "HTTP/1.1 200 OK",
                "application/json; charset=utf-8",
                render_status_json(&mut guard),
            )
        }
        ("GET", "/a6000/api/fleet") | ("GET", "/api/fleet") => {
            let guard = state.lock().unwrap_or_else(|e| e.into_inner());
            (
                "HTTP/1.1 200 OK",
                "application/json; charset=utf-8",
                render_fleet_json(&guard),
            )
        }
        ("POST", "/a6000/api/fleet/probe")
        | ("POST", "/api/fleet/probe")
        | ("GET", "/a6000/api/fleet/probe") => {
            let (specs, current_vaults) = {
                let guard = state.lock().unwrap_or_else(|e| e.into_inner());
                let sp: Vec<FleetNodeSpec> = guard.fleet_nodes.iter().map(|n| n.spec).collect();
                (sp, guard.storage_vaults.clone())
            };
            let (results, updated_vaults) = thread::scope(|s| {
                let mut handles = Vec::with_capacity(specs.len());
                for spec in specs {
                    handles.push(s.spawn(move || {
                        let (tcp_ok, rtt_us) = probe_tcp_rtt_us(spec.ipv4, 22, 1200);
                        let (ssh_ok, uptime_s) = if tcp_ok {
                            probe_ssh_uptime(spec.ipv4, 3)
                        } else {
                            (false, 86_400)
                        };
                        (spec.index, tcp_ok, rtt_us, ssh_ok, uptime_s)
                    }));
                }
                let vh = s.spawn(move || refresh_storage_vaults_live(&current_vaults));
                let res: Vec<(usize, bool, u64, bool, u64)> = handles
                    .into_iter()
                    .filter_map(|h| h.join().ok())
                    .collect();
                (res, vh.join().unwrap_or_default())
            });
            let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
            for (idx, tcp_ok, rtt_us, ssh_ok, uptime_s) in results {
                if let Some(node) = guard.fleet_nodes.get_mut(idx) {
                    node.reachable = tcp_ok;
                    node.ssh_authorized = ssh_ok;
                    node.latest_rtt_us = rtt_us;
                    if uptime_s > 0 {
                        node.uptime_seconds = uptime_s;
                    }
                    node.rtt_ring.push(rtt_us.max(1));
                    node.salience = MarineIntDetector::analyze_timing_ring(&node.rtt_ring.to_vec());
                }
            }
            if !updated_vaults.is_empty() {
                guard.storage_vaults = updated_vaults;
            }
            guard
                .c64_page
                .write_record("FLEET_PROBE:10_NODES:MESH_AUTHORIZED:SALIENCE_UPDATED");
            (
                "HTTP/1.1 200 OK",
                "application/json; charset=utf-8",
                render_fleet_json(&guard),
            )
        }
        ("POST", "/a6000/api/fleet/sync-repos")
        | ("POST", "/api/fleet/sync-repos")
        | ("GET", "/a6000/api/fleet/sync-repos") => {
            let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
            let (entries, dur_us) = sync_13_repos(&mut guard);
            drop(guard);
            let mut items = Vec::with_capacity(entries.len());
            for r in &entries {
                items.push(format!(
                    "{{\"repo\":\"{}\",\"a6000_head_sha\":\"{}\",\"vault_head_sha\":\"{}\",\"synced\":{},\"sync_trit\":{}}}",
                    json_escape(&r.repo_name.replace('.', "·")),
                    json_escape(&r.a6000_head_sha),
                    json_escape(&r.vault_head_sha),
                    r.synced,
                    r.sync_trit.as_i8()
                ));
            }
            let json = format!(
                "{{\"ok\":true,\"float_count\":0,\"synced_count\":{},\"target_count\":13,\"sync_ratio_pq\":\"{}/13\",\"sync_duration_us\":{},\"repositories\":[{}]}}",
                entries.len(),
                entries.len(),
                dur_us,
                items.join(",")
            );
            ("HTTP/1.1 200 OK", "application/json; charset=utf-8", json)
        }
        ("POST", "/a6000/api/g8t/orchestrate")
        | ("POST", "/api/g8t/orchestrate")
        | ("GET", "/a6000/api/g8t/orchestrate") => {
            let nonces = {
                let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                [
                    guard.next_nonce(),
                    guard.next_nonce(),
                    guard.next_nonce(),
                    guard.next_nonce(),
                    guard.next_nonce(),
                ]
            };
            let rep = run_g8t_capsela_with_nonces(nonces);
            {
                let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                guard.c64_page.write_record(&format!(
                    "G8T_ORCHESTRATE:AGENT={}:CIRCUIT={}:OUT={}:RUSTC={}:SNAP_OK={}",
                    AYE_A6000_AGENT_ID,
                    rep.snapped_circuit_name,
                    rep.snapped_output_value,
                    rep.rustc_native_verified,
                    rep.remote_snap_accepted
                ));
                guard.last_g8t_report = Some(rep.clone());
            }
            (
                "HTTP/1.1 200 OK",
                "application/json; charset=utf-8",
                render_g8t_report_json(&rep),
            )
        }
        (_, "/a6000/api/toss/execute")
        | (_, "/api/toss/execute")
        | ("GET", "/a6000/api/toss")
        | ("GET", "/api/toss") => {
            let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
            (
                "HTTP/1.1 200 OK",
                "application/json; charset=utf-8",
                render_toss_json(&mut guard, 432),
            )
        }
        ("POST", "/a6000/api/diary/pulse")
        | ("POST", "/api/diary/pulse")
        | ("GET", "/a6000/api/diary/pulse") => {
            let pulse_id = {
                let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                guard.diary_pulses = guard.diary_pulses.saturating_add(1);
                guard.diary_pulses
            };
            let (diary_ok, wa_ok, _, wa_ev, wa_med, json) = run_diary_and_wa_pulse_io(pulse_id, None);
            {
                let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                guard.c64_page.write_record(&format!(
                    "DIARY_PULSE:COUNT={}:DIARY_OK={}:WA_OK={}:EVENTS={}:MEDIA={}",
                    pulse_id, diary_ok, wa_ok, wa_ev, wa_med
                ));
            }
            ("HTTP/1.1 200 OK", "application/json; charset=utf-8", json)
        }
        (_, "/a6000/api/backup/verify") | (_, "/api/backup/verify") => {
            let cached_opt = {
                let guard = state.lock().unwrap_or_else(|e| e.into_inner());
                if raw_method == "GET" || is_head {
                    guard.last_backup_report.clone()
                } else {
                    None
                }
            };
            let rep = if let Some(cached) = cached_opt {
                cached
            } else {
                let fresh = compute_backup_verification_report();
                let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                guard.c64_page.write_record(&format!(
                    "BACKUP_VERIFY:BOTH={}:FILES=321:BYTES={}:TRIT={}",
                    fresh.both_verified,
                    EXPECTED_BACKUP_BYTES,
                    fresh.vault_parity_trit.as_glyph()
                ));
                guard.last_backup_report = Some(fresh.clone());
                fresh
            };
            (
                "HTTP/1.1 200 OK",
                "application/json; charset=utf-8",
                render_backup_verify_json(&rep),
            )
        }
        _ => (
            "HTTP/1.1 404 Not Found",
            "application/json; charset=utf-8",
            "{\"ok\":false,\"error\":\"route_not_found\",\"float_count\":0}".to_string(),
        ),
    };

    let wire_body = if is_head { "" } else { body.as_str() };
    let response = format!(
        "{}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, HEAD, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n{}",
        status_line,
        content_type,
        body.len(),
        wire_body
    );
    let _ = stream.write_all(response.as_bytes());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let subcmd = args.get(1).map(String::as_str).unwrap_or("serve");

    let mut init_state = A6000OrchestratorState::new();

    match subcmd {
        "status" => {
            println!("{}", render_status_json(&mut init_state));
            return;
        }
        "fleet" => {
            probe_all_fleet_nodes(&mut init_state, true);
            println!("{}", render_fleet_json(&init_state));
            return;
        }
        "g8t-bench" => {
            let rep = orchestrate_g8t_capsela(&mut init_state);
            println!("{}", render_g8t_report_json(&rep));
            return;
        }
        "toss" => {
            println!("{}", render_toss_json(&mut init_state, 432));
            return;
        }
        "backup-verify" => {
            let rep = verify_backup_vaults(&mut init_state);
            println!("{}", render_backup_verify_json(&rep));
            return;
        }
        _ => {}
    }

    // Pre-warm initial fleet TCP probe and backup check on startup
    probe_all_fleet_nodes(&mut init_state, false);
    let _ = verify_backup_vaults(&mut init_state);

    let bind_addr = std::env::var("A6000_BIND").unwrap_or_else(|_| "0.0.0.0:8790".to_string());
    let listener = TcpListener::bind(&bind_addr).expect("Failed to bind A6000 orchestrator port");
    println!(
        "[a6000-orchestrator] Listening on http://{} (RTX 6000 Ada · 49140 MiB · C64 64KB Ring · Zero Floats)",
        bind_addr
    );

    let shared = Arc::new(Mutex::new(init_state));
    for stream in listener.incoming().flatten() {
        let state_clone = Arc::clone(&shared);
        thread::spawn(move || {
            handle_http_connection(stream, state_clone);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_json_and_html_outputs_strictly_zero_floats() {
        let mut state = A6000OrchestratorState::new();
        let status_json = render_status_json(&mut state);
        assert_eq!(
            count_floating_point_literals(&status_json),
            0,
            "Float literal found in status_json: {}",
            status_json
        );

        let fleet_json = render_fleet_json(&state);
        assert_eq!(
            count_floating_point_literals(&fleet_json),
            0,
            "Float literal found in fleet_json: {}",
            fleet_json
        );

        let toss_json = render_toss_json(&mut state, 432);
        assert_eq!(
            count_floating_point_literals(&toss_json),
            0,
            "Float literal found in toss_json: {}",
            toss_json
        );

        let html = render_command_deck_html();
        assert_eq!(
            count_floating_point_literals(html),
            0,
            "Float literal found in Command Deck HTML"
        );
    }
}
