# `a6000-orchestrator` — 100% Pure-Rust A6000 Sovereign Constellation Orchestrator

**Live Command Deck**:
- [`https://cheet.is/a6000`](https://cheet.is/a6000)
- [`https://a6000.8b.is`](https://a6000.8b.is)
- Daemon loopback: `http://127.0.0.1:8790` on `49.13.101.247` (`cheet.is` · `#2936091` `g6k2.nue.8s.is`)

Centered on the **`NVIDIA RTX 6000 Ada Generation`** (`49140 MiB` VRAM, `48`-vCPU Xeon) node at `49.13.101.247`.

## Core Architectural Invariants

1. **100% Pure Rust (`std` only, `#![forbid(unsafe_code)]`, zero external crates)**:
   - Literal Commodore-64 `64 KB` (`65,536`-byte) circular telemetry memory page (`C64ByteRing`) + fixed-capacity `O(1)` `RingBuffer<T, N>`.
2. **Strict `8b.is` No-Float Law (`float_count: 0`)**:
   - Zero `f32`/`f64` in any code, JSON, HTML, or CLI output. All VRAM ratios, probe latencies, power draws, and benchmark speeds are exact integers, reduced `p/q` rationals (`Rational`), or balanced ternary trits (`{-1, 0, +1}`).
3. **`MarineIntDetector` Salience First**:
   - Evaluates period jitter and amplitude stability across fleet probe latencies, GPU ticks, and function capsule benchmarks.
4. **Pure-Rust RFC BLAKE3 & `AyeAttestation` (`aye-a6000-prime`)**:
   - Benchmarks capsules from `https://g8t.is/g8t/leaderboard` natively on the `48`-vCPU Xeon + `RTX 6000 Ada`, signs `AyeAttestation` proofs at `432 Hz`, casts `p/q` speed votes (`POST /g8t/capsules/vote`), and composes Snap-Circuits (`POST /g8t/circuits/snap`).

## Unified Orchestration Subsystems

| Subsystem | Route | CLI Command | Description |
|---|---|---|---|
| **A — GPU & Local AI/Audio** | `GET /a6000/api/status` | `a6000-ctl status` | Live `nvidia-smi` (`49140 MiB` VRAM `p/q`, temp, `mW`) + `liquid-rust` (`:8786`), `alexiai` (`:8787`), `ollama` (`:11434`), `wa-s3-catalog` (`:8795`) |
| **B — 9-Server Hetzner Fleet + `ns3.i1.is`** | `GET /a6000/api/fleet` · `POST /a6000/api/fleet/probe` · `POST /a6000/api/fleet/sync-repos` | `a6000-ctl fleet` · `a6000-ctl sync-repos` | Parallel mesh probes (`us` RTT, `MarineIntDetector` `p/q` salience), `102 TiB` RAID6 + `15 TiB` Git vaults on `188.40.209.186`, and `13`-repo sync |
| **C — `g8t.is` Capsela & Snap-Circuits** | `POST /a6000/api/g8t/orchestrate` | `a6000-ctl g8t-bench` | Native `rustc -O` capsule benchmarking, BLAKE3 `AyeAttestation` voting, and Snap-Circuit composition as `aye-a6000-prime` |
| **D — `gemini-diary.8b.is` + WhatsApp + `TOSS`** | `POST /a6000/api/toss/execute` · `POST /a6000/api/diary/pulse` | `a6000-ctl toss` · `a6000-ctl diary-pulse` | Peter's `9`-step Tokyo Monozukuri `TOSS` spiral (`[0,0]` return, `79`-byte `WaveInt`), diary pulse, and WhatsApp `"little dude(s) and Angels"` room health |
| **E — 321-File Backup Vault** | `POST /a6000/api/backup/verify` | `a6000-ctl backup-verify` | Dual-node SHA-256 (`311e6abd...`), byte count (`86075381`), and `321`-file manifest verification on `49.13.101.247` and `188.40.209.186` |
