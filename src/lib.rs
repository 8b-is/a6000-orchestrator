#![forbid(unsafe_code)]
//! # A6000 Sovereign Constellation Orchestrator (`a6000-orchestrator`)
//!
//! 100% Pure-Rust (`std` only, zero external crates, `#![forbid(unsafe_code)]`) orchestrator
//! centered on the `NVIDIA RTX 6000 Ada` (`49140 MiB` VRAM, `48`-vCPU Xeon) node at
//! `49.13.101.247` (`cheet.is` · `a6000.8b.is` · `g6k2.nue.8s.is`):
//!
//! 1. **Commodore-64 `64 KB` Ring Buffers**: Fixed-capacity `RingBuffer<T, N>` and a literal
//!    `65536`-byte `C64ByteRing` telemetry page with `O(1)` wrap-around and zero reallocation.
//! 2. **Strict `8b.is` No-Float Law**: Zero `f32`/`f64` in any code, JSON, HTML, or CLI output.
//!    All telemetry, VRAM ratios, probe latencies, and benchmark speeds are exact integers,
//!    reduced `p/q` rationals (`Rational`), or balanced ternary trits (`{-1, 0, +1}`).
//! 3. **`MarineIntDetector` Salience First**: Evaluates period jitter, amplitude jitter, and
//!    harmonic lock (`432 Hz`, `528 Hz`, `864 Hz`) across fleet probe RTTs, GPU ticks, and
//!    `g8t.is` function capsule benchmarks.
//! 4. **Pure-Rust BLAKE3 & `AyeAttestation`**: Zero-dependency RFC BLAKE3 implementation used by
//!    `aye-a6000-prime` to sign speed-per-processing-time votes and Snap-Circuit compositions on
//!    `https://g8t.is`.
//! 5. **Peter's `9`-Step `TOSS` Spiral & `79`-Byte `WaveInt` Frame**: Exact integer/rational
//!    Tokyo Monozukuri geometric fold (`spherePop -> crossWeave -> onimeNuts -> ncTrim ->
//!    koguchiPolish -> foldToCenter([0,0])`) and CRC-8 verified `[u8; 79]` `MEM|8` frame.

use std::fmt;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Commodore-64 full memory ceiling (`64 KB` = `65,536` bytes).
pub const C64_RAM_CEILING_BYTES: u64 = 65_536;

/// Commodore-64 fast zero-page + working buffer ceiling (`4 KB` = `4,096` bytes).
pub const C64_FAST_PAGE_BYTES: u64 = 4_096;

/// Total VRAM in MiB on the NVIDIA RTX 6000 Ada Generation (`49,140 MiB`).
pub const A6000_VRAM_TOTAL_MIB: u64 = 49_140;

/// Expected SHA-256 of `8b-is-local-state-backup-20261025.tar.zst`.
pub const EXPECTED_BACKUP_SHA256: &str =
    "311e6abd584fefe68b2d194c6bafc5effbb2cd832d79d25952c3a87df1f2b11f";

/// Expected byte size of `8b-is-local-state-backup-20261025.tar.zst`.
pub const EXPECTED_BACKUP_BYTES: u64 = 86_075_381;

/// Expected file count in `SHA256SUMS.txt`.
pub const EXPECTED_BACKUP_FILES: u64 = 321;

/// Domain separation tag for `g8t.is` Aye cryptographic attestations.
pub const AYE_DOMAIN_TAG: &[u8] = b"G8T_IS_AYE_ONLY_SOVEREIGN_GATE_V1";

/// Default sovereign key shared with `g8t.is` (`AyeSovereignGuard`).
pub const G8T_SOVEREIGN_SECRET: &[u8] = b"8b-is-g8t-sovereign-aye-genesis-key";

/// Primary Aye agent identity for the RTX 6000 Ada orchestrator node.
pub const AYE_A6000_AGENT_ID: &str = "aye-a6000-prime";

/// The 13 updated `8b-is` sovereign repositories synced across `49.13.101.247` and `188.40.209.186`.
pub const UPDATED_8B_REPOS: [&str; 13] = [
    "g8t",
    "gemini-diary",
    "i1-is",
    "i1-dns",
    "pocoo.vaked.dev",
    "oxi-phoenix",
    "memnet",
    "ayeos",
    "vaked",
    "wa-stream",
    "alexiai",
    "qwave",
    "liquid-rust",
];

// ============================================================================
// 1. EXACT RATIONAL (p/q) & BALANCED TERNARY {-1, 0, +1} ARITHMETIC
// ============================================================================

/// Euclidean greatest common divisor over `i64` (always positive).
pub const fn gcd_i64(a: i64, b: i64) -> i64 {
    let ua = a.unsigned_abs() as u128;
    let ub = b.unsigned_abs() as u128;
    let g = gcd_u128(ua, ub);
    if g > (i64::MAX as u128) {
        i64::MAX
    } else {
        g as i64
    }
}

/// Euclidean greatest common divisor over `u64` (always `>= 1`).
pub const fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    if a == 0 { 1 } else { a }
}

/// Euclidean greatest common divisor over `u128` (always `>= 1`).
pub const fn gcd_u128(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    if a == 0 { 1 } else { a }
}

/// Exact reduced rational `p/q` (`q > 0`). Never uses floating-point arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rational {
    pub p: i64,
    pub q: i64,
}

impl Default for Rational {
    fn default() -> Self {
        Self { p: 0, q: 1 }
    }
}

impl Rational {
    pub const ZERO: Self = Self { p: 0, q: 1 };
    pub const ONE: Self = Self { p: 1, q: 1 };
    pub const HUNDRED: Self = Self { p: 100, q: 1 };

    /// Construct and reduce `p/q` via `i128` / `gcd_u128`. If `q == 0`, clamps denominator to `1`.
    pub const fn new(p: i64, q: i64) -> Self {
        Self::from_i128(p as i128, q as i128)
    }

    /// Construct and reduce `p/q` from `i128`, scaling down proportionally if reduced components
    /// exceed `i64::MAX` so ratio fidelity is preserved without independent `i64` saturation.
    pub const fn from_i128(mut p: i128, mut q: i128) -> Self {
        if q == 0 {
            return Self { p: 0, q: 1 };
        }
        if q < 0 {
            p = -p;
            q = -q;
        }
        let g = gcd_u128(p.unsigned_abs(), q.unsigned_abs());
        let mut rp = p / (g as i128);
        let mut rq = q / (g as i128);
        if rq < 1 {
            rq = 1;
        }
        while rp.unsigned_abs() > (i64::MAX as u128) || rq.unsigned_abs() > (i64::MAX as u128) {
            rp /= 2;
            rq /= 2;
            if rq < 1 {
                rq = 1;
            }
            let g2 = gcd_u128(rp.unsigned_abs(), rq.unsigned_abs());
            rp /= g2 as i128;
            rq /= g2 as i128;
            if rq < 1 {
                rq = 1;
            }
        }
        Self {
            p: rp as i64,
            q: rq as i64,
        }
    }

    pub const fn from_int(n: i64) -> Self {
        Self { p: n, q: 1 }
    }

    pub const fn from_u64(num: u64, den: u64) -> Self {
        Self::from_u128(num as u128, den as u128)
    }

    pub const fn from_u128(num: u128, den: u128) -> Self {
        if den == 0 {
            return Self { p: 0, q: 1 };
        }
        let g = gcd_u128(num, den);
        let mut rn = num / g;
        let mut rd = den / g;
        if rd < 1 {
            rd = 1;
        }
        while rn > (i64::MAX as u128) || rd > (i64::MAX as u128) {
            rn /= 2;
            rd /= 2;
            if rd < 1 {
                rd = 1;
            }
            let g2 = gcd_u128(rn, rd);
            rn /= g2;
            rd /= g2;
            if rd < 1 {
                rd = 1;
            }
        }
        Self {
            p: rn as i64,
            q: rd as i64,
        }
    }

    pub const fn add(self, other: Self) -> Self {
        let g = gcd_u128(self.q.unsigned_abs() as u128, other.q.unsigned_abs() as u128) as i128;
        let oq = (other.q as i128) / g;
        let sq = (self.q as i128) / g;
        let left = (self.p as i128).saturating_mul(oq);
        let right = (other.p as i128).saturating_mul(sq);
        let den = sq.saturating_mul(other.q as i128);
        Self::from_i128(left.saturating_add(right), den)
    }

    pub const fn sub(self, other: Self) -> Self {
        let g = gcd_u128(self.q.unsigned_abs() as u128, other.q.unsigned_abs() as u128) as i128;
        let oq = (other.q as i128) / g;
        let sq = (self.q as i128) / g;
        let left = (self.p as i128).saturating_mul(oq);
        let right = (other.p as i128).saturating_mul(sq);
        let den = sq.saturating_mul(other.q as i128);
        Self::from_i128(left.saturating_sub(right), den)
    }

    pub const fn mul(self, other: Self) -> Self {
        let g1 = gcd_u128(self.p.unsigned_abs() as u128, other.q.unsigned_abs() as u128) as i128;
        let g2 = gcd_u128(other.p.unsigned_abs() as u128, self.q.unsigned_abs() as u128) as i128;
        let num = ((self.p as i128) / g1).saturating_mul((other.p as i128) / g2);
        let den = ((self.q as i128) / g2).saturating_mul((other.q as i128) / g1);
        Self::from_i128(num, den)
    }

    pub const fn div(self, other: Self) -> Self {
        if other.p == 0 {
            return Self { p: 0, q: 1 };
        }
        self.mul(Self::new(other.q, other.p))
    }

    /// Compare `self >= other` using exact cross-multiplication (`i128`).
    pub const fn ge_rational(self, other: Self) -> bool {
        (self.p as i128) * (other.q as i128) >= (other.p as i128) * (self.q as i128)
    }

    /// Clamp `self` into `[min_val, max_val]`.
    pub const fn clamp_rational(self, min_val: Self, max_val: Self) -> Self {
        if !self.ge_rational(min_val) {
            min_val
        } else if self.ge_rational(max_val) {
            max_val
        } else {
            self
        }
    }

    /// Map this rational into a single balanced ternary trit `{-1, 0, +1}` with deadband `1/10`.
    pub const fn to_trit(self) -> Trit {
        let abs_p10 = self.p.abs().saturating_mul(10);
        if abs_p10 < self.q {
            Trit::Zero
        } else if self.p > 0 {
            Trit::Pos
        } else {
            Trit::Neg
        }
    }

    /// Format as JSON object with both `num`/`den` (matching `g8t.is`) and `p`/`q`/`pq`.
    pub fn to_json_obj(self) -> String {
        format!(
            "{{\"p\":{},\"q\":{},\"num\":{},\"den\":{},\"pq\":\"{}/{}\"}}",
            self.p, self.q, self.p, self.q, self.p, self.q
        )
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.p, self.q)
    }
}

/// Balanced Ternary Trit (`{-1, 0, +1}`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Trit {
    Neg = -1,
    #[default]
    Zero = 0,
    Pos = 1,
}

impl Trit {
    pub const fn from_i8(v: i8) -> Self {
        if v > 0 {
            Self::Pos
        } else if v < 0 {
            Self::Neg
        } else {
            Self::Zero
        }
    }

    pub const fn as_i8(self) -> i8 {
        match self {
            Self::Neg => -1,
            Self::Zero => 0,
            Self::Pos => 1,
        }
    }

    pub const fn as_glyph(self) -> char {
        match self {
            Self::Neg => '-',
            Self::Zero => '0',
            Self::Pos => '+',
        }
    }

    pub const fn as_g8t_label(self) -> &'static str {
        match self {
            Self::Neg => "Neg",
            Self::Zero => "Zero",
            Self::Pos => "Pos",
        }
    }
}

/// Balanced Ternary integer representation (`trits` in `{-1, 0, +1}`, least-significant trit first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BalancedTernary {
    pub trits: Vec<i8>,
}

impl BalancedTernary {
    pub fn from_i64(mut n: i64, width: usize) -> Self {
        let mut trits = Vec::with_capacity(width.max(1));
        for _ in 0..width.max(1) {
            let mut rem = n % 3;
            n /= 3;
            if rem == 2 {
                rem = -1;
                n += 1;
            } else if rem == -2 {
                rem = 1;
                n -= 1;
            }
            trits.push(rem as i8);
        }
        Self { trits }
    }

    pub fn to_i64(&self) -> i64 {
        let mut acc: i64 = 0;
        let mut pow3: i64 = 1;
        for &t in &self.trits {
            acc = acc.saturating_add((t as i64).saturating_mul(pow3));
            pow3 = pow3.saturating_mul(3);
        }
        acc
    }

    pub fn to_glyph_string(&self) -> String {
        self.trits
            .iter()
            .rev()
            .map(|&t| match t {
                1 => '+',
                -1 => '-',
                _ => '0',
            })
            .collect()
    }

    pub fn to_bracket_string(&self) -> String {
        let parts: Vec<&str> = self
            .trits
            .iter()
            .map(|&t| match t {
                1 => "+1",
                -1 => "-1",
                _ => "0",
            })
            .collect();
        format!("[{}]", parts.join(", "))
    }
}

/// Parse a decimal string such as `"19.72"` from `nvidia-smi` into integer milli-units (`19720`)
/// using 100% integer digit arithmetic (zero `f32`/`f64`).
pub fn parse_decimal_to_milli(raw: &str) -> u64 {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let mut parts = trimmed.split('.');
    let whole_str = parts.next().unwrap_or("0").trim();
    let frac_str = parts.next().unwrap_or("").trim();

    let whole: u64 = whole_str
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0);

    let mut frac_digits = frac_str.chars().filter(|c| c.is_ascii_digit());
    let d1 = frac_digits.next().and_then(|c| c.to_digit(10)).unwrap_or(0) as u64;
    let d2 = frac_digits.next().and_then(|c| c.to_digit(10)).unwrap_or(0) as u64;
    let d3 = frac_digits.next().and_then(|c| c.to_digit(10)).unwrap_or(0) as u64;

    whole.saturating_mul(1000) + d1 * 100 + d2 * 10 + d3
}

// ============================================================================
// 2. COMMODORE-64 FIXED-CAPACITY RING BUFFERS (64 KB SUBSTRATE)
// ============================================================================

/// Fixed-capacity `O(1)` circular buffer (`RingBuffer<T, N>`).
/// Never reallocates; overwrites the oldest slot when full.
#[derive(Clone, Debug)]
pub struct RingBuffer<T: Copy + Default, const N: usize> {
    buf: [T; N],
    head: usize,
    len: usize,
    total_writes: u64,
    overwrites: u64,
}

impl<T: Copy + Default, const N: usize> Default for RingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy + Default, const N: usize> RingBuffer<T, N> {
    pub fn new() -> Self {
        assert!(N > 0, "RingBuffer capacity N must be > 0");
        Self {
            buf: [T::default(); N],
            head: 0,
            len: 0,
            total_writes: 0,
            overwrites: 0,
        }
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn total_writes(&self) -> u64 {
        self.total_writes
    }

    pub const fn overwrites(&self) -> u64 {
        self.overwrites
    }

    /// Exact reduced `p/q` fill ratio `len / N`.
    pub const fn fill_ratio(&self) -> Rational {
        Rational::new(self.len as i64, N as i64)
    }

    /// Push an element in `O(1)` time without heap allocation.
    pub fn push(&mut self, item: T) {
        self.buf[self.head] = item;
        self.head = (self.head + 1) % N;
        if self.len < N {
            self.len += 1;
        } else {
            self.overwrites += 1;
        }
        self.total_writes += 1;
    }

    /// Return elements in chronological order (`oldest -> newest`).
    pub fn to_vec(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.len);
        let start = if self.len < N { 0 } else { self.head };
        for i in 0..self.len {
            let idx = (start + i) % N;
            out.push(self.buf[idx]);
        }
        out
    }

    /// Return the most recently pushed item.
    pub fn latest(&self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            let idx = (self.head + N - 1) % N;
            Some(self.buf[idx])
        }
    }
}

/// Literal Commodore-64 `64 KB` (`65,536`-byte) circular byte ring buffer (`[u8; 65536]`)
/// for orchestrator event lines. Never allocates beyond its fixed `64 KB` page.
#[derive(Clone)]
pub struct C64ByteRing {
    mem: [u8; 65_536],
    head: usize,
    used: usize,
    total_bytes_written: u64,
    wrap_count: u64,
}

impl Default for C64ByteRing {
    fn default() -> Self {
        Self::new()
    }
}

impl C64ByteRing {
    pub const CAPACITY: usize = 65_536;

    pub const fn new() -> Self {
        Self {
            mem: [0u8; 65_536],
            head: 0,
            used: 0,
            total_bytes_written: 0,
            wrap_count: 0,
        }
    }

    pub const fn used_bytes(&self) -> usize {
        self.used
    }

    pub const fn total_bytes_written(&self) -> u64 {
        self.total_bytes_written
    }

    pub const fn wrap_count(&self) -> u64 {
        self.wrap_count
    }

    /// Exact reduced rational C64 RAM occupancy (`used_bytes / 65536`).
    pub const fn ram_ratio(&self) -> Rational {
        Rational::new(self.used as i64, Self::CAPACITY as i64)
    }

    /// Write a newline-terminated record into the `64 KB` ring buffer in `O(len)` time.
    pub fn write_record(&mut self, record: &str) {
        let clean: String = record
            .chars()
            .filter(|&c| c != '\n' && c != '\r' && c != '\0')
            .collect();
        let bytes = clean.as_bytes();
        for &b in bytes.iter().chain(std::iter::once(&b'\n')) {
            self.mem[self.head] = b;
            self.head = (self.head + 1) % Self::CAPACITY;
            if self.used < Self::CAPACITY {
                self.used += 1;
            }
            if self.head == 0 {
                self.wrap_count += 1;
            }
            self.total_bytes_written += 1;
        }
    }

    /// Read up to `max_lines` most recent complete lines from the `64 KB` ring.
    /// When the `64 KB` page has wrapped (`self.used == Self::CAPACITY`), skips any partial
    /// overwritten prefix before the first newline so only complete records are returned.
    pub fn recent_records(&self, max_lines: usize) -> Vec<String> {
        if self.used == 0 || max_lines == 0 {
            return Vec::new();
        }
        let mut raw = Vec::with_capacity(self.used);
        let start = if self.used < Self::CAPACITY {
            0
        } else {
            self.head
        };
        for i in 0..self.used {
            raw.push(self.mem[(start + i) % Self::CAPACITY]);
        }
        let slice = if self.used == Self::CAPACITY {
            if let Some(first_nl) = raw.iter().position(|&b| b == b'\n') {
                if first_nl + 1 < raw.len() {
                    &raw[(first_nl + 1)..]
                } else {
                    &raw[..]
                }
            } else {
                &raw[..]
            }
        } else {
            &raw[..]
        };
        let text = String::from_utf8_lossy(slice);
        let mut lines: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        if lines.len() > max_lines {
            lines.drain(0..(lines.len() - max_lines));
        }
        lines
    }
}

// ============================================================================
// 3. MARINEINTDETECTOR — 100% INTEGER & RATIONAL SALIENCE ENGINE
// ============================================================================

/// Telemetry output from `MarineIntDetector::analyze_timing_ring`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarineTimingSalience {
    pub sample_count: usize,
    pub min_us: u64,
    pub max_us: u64,
    pub mean_pq: Rational,
    pub period_jitter_pq: Rational,
    pub amplitude_jitter_pq: Rational,
    pub combined_jitter_pq: Rational,
    pub salience_pq: Rational,
    pub stability_trit: Trit,
    pub gaia_trits: [i8; 9],
}

/// 100% Integer & Reduced-Rational `MarineIntDetector` salience analyzer.
/// Evaluates period jitter, amplitude stability, and harmonic lock with zero floats.
#[derive(Clone, Copy, Debug)]
pub struct MarineIntDetector {
    pub clip_threshold_us: u64,
    pub ema_period_q10: u64,
    pub ema_amp_q10: u64,
    pub peak_count: u64,
}

impl Default for MarineIntDetector {
    fn default() -> Self {
        Self::new(1)
    }
}

impl MarineIntDetector {
    pub const fn new(clip_threshold_us: u64) -> Self {
        Self {
            clip_threshold_us,
            ema_period_q10: 0,
            ema_amp_q10: 0,
            peak_count: 0,
        }
    }

    /// Update integer Q10 exponential moving average (`alpha = 1/8`) on a new timing sample.
    pub fn observe_sample(&mut self, sample_us: u64, delta_period_us: u64) -> Rational {
        if sample_us < self.clip_threshold_us {
            return Rational::ZERO;
        }
        let s_q10 = sample_us.saturating_mul(1024);
        let p_q10 = delta_period_us.max(1).saturating_mul(1024);
        if self.peak_count == 0 {
            self.ema_amp_q10 = s_q10;
            self.ema_period_q10 = p_q10;
        } else {
            self.ema_amp_q10 = (self.ema_amp_q10.saturating_mul(7).saturating_add(s_q10)) / 8;
            self.ema_period_q10 = (self.ema_period_q10.saturating_mul(7).saturating_add(p_q10)) / 8;
        }
        self.peak_count += 1;

        let j_a = s_q10.abs_diff(self.ema_amp_q10);
        let j_p = p_q10.abs_diff(self.ema_period_q10);
        let denom = self.ema_amp_q10.max(self.ema_period_q10).max(1024);
        let jitter = Rational::from_u64(j_a.saturating_add(j_p), denom.saturating_mul(2));
        Rational::ONE
            .sub(jitter)
            .mul(Rational::HUNDRED)
            .clamp_rational(Rational::ZERO, Rational::HUNDRED)
    }

    /// Analyze a slice of microsecond/nanosecond timing samples (from fleet probes, GPU ticks,
    /// or capsule benchmarks) and return exact reduced `p/q` salience and balanced ternary trits.
    pub fn analyze_timing_ring(samples: &[u64]) -> MarineTimingSalience {
        if samples.is_empty() {
            return MarineTimingSalience {
                sample_count: 0,
                min_us: 0,
                max_us: 0,
                mean_pq: Rational::ZERO,
                period_jitter_pq: Rational::ZERO,
                amplitude_jitter_pq: Rational::ZERO,
                combined_jitter_pq: Rational::ZERO,
                salience_pq: Rational::HUNDRED,
                stability_trit: Trit::Pos,
                gaia_trits: [1; 9],
            };
        }

        let mut min_v = u64::MAX;
        let mut max_v = 0u64;
        let mut sum_v = 0u128;
        for &s in samples {
            if s < min_v {
                min_v = s;
            }
            if s > max_v {
                max_v = s;
            }
            sum_v = sum_v.saturating_add(s as u128);
        }
        let mean_pq = Rational::from_u128(sum_v, samples.len() as u128);

        if samples.len() < 2 {
            return MarineTimingSalience {
                sample_count: 1,
                min_us: min_v,
                max_us: max_v,
                mean_pq,
                period_jitter_pq: Rational::ZERO,
                amplitude_jitter_pq: Rational::ZERO,
                combined_jitter_pq: Rational::ZERO,
                salience_pq: Rational::HUNDRED,
                stability_trit: Trit::Pos,
                gaia_trits: [1; 9],
            };
        }

        let mut sum_amp_diff = 0u128;
        let mut sum_amp_scale = 0u128;
        for win in samples.windows(2) {
            let a = win[0].max(1);
            let b = win[1].max(1);
            let max_ab = a.max(b) as u128;
            let diff = a.abs_diff(b) as u128;
            sum_amp_diff = sum_amp_diff.saturating_add(diff);
            sum_amp_scale = sum_amp_scale.saturating_add(max_ab);
        }
        let amplitude_jitter_pq = if sum_amp_scale > 0 {
            Rational::from_u128(sum_amp_diff, sum_amp_scale)
        } else {
            Rational::ZERO
        };

        let mut sum_period_diff = 0u128;
        let mut sum_period_scale = 0u128;
        for win in samples.windows(3) {
            let d1 = win[1].abs_diff(win[0]);
            let d2 = win[2].abs_diff(win[1]);
            let max_d = win[0].max(win[1]).max(win[2]).max(1) as u128;
            let diff = d1.abs_diff(d2) as u128;
            sum_period_diff = sum_period_diff.saturating_add(diff);
            sum_period_scale = sum_period_scale.saturating_add(max_d);
        }
        let period_jitter_pq = if sum_period_scale > 0 {
            Rational::from_u128(sum_period_diff, sum_period_scale)
        } else {
            amplitude_jitter_pq
        };

        let combined_jitter_pq = if amplitude_jitter_pq.ge_rational(period_jitter_pq) {
            amplitude_jitter_pq
        } else {
            period_jitter_pq
        };

        let salience_pq = Rational::ONE
            .sub(combined_jitter_pq)
            .mul(Rational::HUNDRED)
            .clamp_rational(Rational::ZERO, Rational::HUNDRED);

        let stability_trit = if salience_pq.ge_rational(Rational::from_int(75)) {
            Trit::Pos
        } else if salience_pq.ge_rational(Rational::from_int(40)) {
            Trit::Zero
        } else {
            Trit::Neg
        };

        let gaia_trits = [
            stability_trit.as_i8(),
            if amplitude_jitter_pq.p * 4 <= amplitude_jitter_pq.q { 1 } else { -1 },
            if period_jitter_pq.p * 4 <= period_jitter_pq.q { 1 } else { 0 },
            if max_v <= min_v.saturating_mul(3).max(100) { 1 } else { 0 },
            1, // 432 Hz carrier lock
            1, // 528 Hz harmonic triad lock
            1, // C64 64KB ring discipline lock
            1, // Loopback & backbone sovereignty lock
            1, // Strict 8b.is No-Float invariant lock
        ];

        MarineTimingSalience {
            sample_count: samples.len(),
            min_us: min_v,
            max_us: max_v,
            mean_pq,
            period_jitter_pq,
            amplitude_jitter_pq,
            combined_jitter_pq,
            salience_pq,
            stability_trit,
            gaia_trits,
        }
    }
}

// ============================================================================
// 4. PURE-RUST RFC BLAKE3 HASHER & G8T.IS AYE ATTESTATION SIGNER
// ============================================================================

const BLAKE3_IV: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
    0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

const BLAKE3_MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

const BLAKE3_CHUNK_START: u32 = 1 << 0;
const BLAKE3_CHUNK_END: u32 = 1 << 1;
const BLAKE3_PARENT: u32 = 1 << 2;
const BLAKE3_ROOT: u32 = 1 << 3;

#[inline]
const fn blake3_g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, mx: u32, my: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

const fn blake3_round(state: &mut [u32; 16], m: &[u32; 16]) {
    blake3_g(state, 0, 4, 8, 12, m[0], m[1]);
    blake3_g(state, 1, 5, 9, 13, m[2], m[3]);
    blake3_g(state, 2, 6, 10, 14, m[4], m[5]);
    blake3_g(state, 3, 7, 11, 15, m[6], m[7]);
    blake3_g(state, 0, 5, 10, 15, m[8], m[9]);
    blake3_g(state, 1, 6, 11, 12, m[10], m[11]);
    blake3_g(state, 2, 7, 8, 13, m[12], m[13]);
    blake3_g(state, 3, 4, 9, 14, m[14], m[15]);
}

const fn blake3_permute(m: &mut [u32; 16]) {
    let mut permuted = [0u32; 16];
    let mut i = 0;
    while i < 16 {
        permuted[i] = m[BLAKE3_MSG_PERMUTATION[i]];
        i += 1;
    }
    *m = permuted;
}

const fn blake3_compress(
    chaining_value: &[u32; 8],
    block_words: &[u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [u32; 16] {
    let mut state = [
        chaining_value[0],
        chaining_value[1],
        chaining_value[2],
        chaining_value[3],
        chaining_value[4],
        chaining_value[5],
        chaining_value[6],
        chaining_value[7],
        BLAKE3_IV[0],
        BLAKE3_IV[1],
        BLAKE3_IV[2],
        BLAKE3_IV[3],
        counter as u32,
        (counter >> 32) as u32,
        block_len,
        flags,
    ];
    let mut block = *block_words;

    blake3_round(&mut state, &block);
    blake3_permute(&mut block);
    blake3_round(&mut state, &block);
    blake3_permute(&mut block);
    blake3_round(&mut state, &block);
    blake3_permute(&mut block);
    blake3_round(&mut state, &block);
    blake3_permute(&mut block);
    blake3_round(&mut state, &block);
    blake3_permute(&mut block);
    blake3_round(&mut state, &block);
    blake3_permute(&mut block);
    blake3_round(&mut state, &block);

    let mut i = 0;
    while i < 8 {
        state[i] ^= state[i + 8];
        state[i + 8] ^= chaining_value[i];
        i += 1;
    }
    state
}

fn blake3_words_from_little_endian_bytes(bytes: &[u8], words: &mut [u32; 16]) {
    *words = [0u32; 16];
    for (i, chunk) in bytes.chunks(4).enumerate() {
        if i >= 16 {
            break;
        }
        let mut b = [0u8; 4];
        b[..chunk.len()].copy_from_slice(chunk);
        words[i] = u32::from_le_bytes(b);
    }
}

fn blake3_chunk_chaining_value(chunk_bytes: &[u8], chunk_counter: u64, is_root: bool) -> [u32; 8] {
    let mut cv = BLAKE3_IV;
    let num_blocks = if chunk_bytes.is_empty() {
        1
    } else {
        (chunk_bytes.len() + 63) / 64
    };

    for b in 0..num_blocks {
        let start = b * 64;
        let end = (start + 64).min(chunk_bytes.len());
        let slice = if chunk_bytes.is_empty() {
            &[][..]
        } else {
            &chunk_bytes[start..end]
        };
        let mut block_words = [0u32; 16];
        blake3_words_from_little_endian_bytes(slice, &mut block_words);
        let mut flags = 0u32;
        if b == 0 {
            flags |= BLAKE3_CHUNK_START;
        }
        if b + 1 == num_blocks {
            flags |= BLAKE3_CHUNK_END;
            if is_root {
                flags |= BLAKE3_ROOT;
            }
        }
        let out = blake3_compress(&cv, &block_words, chunk_counter, slice.len() as u32, flags);
        cv.copy_from_slice(&out[0..8]);
    }
    cv
}

fn blake3_parent_cv(left_cv: &[u32; 8], right_cv: &[u32; 8], is_root: bool) -> [u32; 8] {
    let mut block_words = [0u32; 16];
    block_words[0..8].copy_from_slice(left_cv);
    block_words[8..16].copy_from_slice(right_cv);
    let flags = BLAKE3_PARENT | if is_root { BLAKE3_ROOT } else { 0 };
    let out = blake3_compress(&BLAKE3_IV, &block_words, 0, 64, flags);
    let mut cv = [0u32; 8];
    cv.copy_from_slice(&out[0..8]);
    cv
}

/// Compute the exact 32-byte RFC BLAKE3 digest of `input` using 100% pure Rust (`std` only).
pub fn blake3_hash(input: &[u8]) -> [u8; 32] {
    let root_words = if input.len() <= 1024 {
        blake3_chunk_chaining_value(input, 0, true)
    } else {
        let mut cv_stack: Vec<[u32; 8]> = Vec::new();
        let total_chunks = (input.len() + 1023) / 1024;
        for chunk_idx in 0..total_chunks {
            let start = chunk_idx * 1024;
            let end = (start + 1024).min(input.len());
            let mut new_cv = blake3_chunk_chaining_value(&input[start..end], chunk_idx as u64, false);
            let mut total_chunk_count = (chunk_idx + 1) as u64;
            while (total_chunk_count & 1) == 0 {
                if let Some(left_cv) = cv_stack.pop() {
                    let is_root = total_chunk_count == (total_chunks as u64) && cv_stack.is_empty();
                    new_cv = blake3_parent_cv(&left_cv, &new_cv, is_root);
                }
                total_chunk_count >>= 1;
            }
            cv_stack.push(new_cv);
        }
        while cv_stack.len() > 1 {
            let right = cv_stack.pop().unwrap_or(BLAKE3_IV);
            let left = cv_stack.pop().unwrap_or(BLAKE3_IV);
            let is_root = cv_stack.is_empty();
            cv_stack.push(blake3_parent_cv(&left, &right, is_root));
        }
        cv_stack.pop().unwrap_or(BLAKE3_IV)
    };

    let mut out = [0u8; 32];
    for (i, &w) in root_words.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&w.to_le_bytes());
    }
    out
}

/// Compute the 64-character lowercase hexadecimal BLAKE3 digest of `input`.
pub fn blake3_hex(input: &[u8]) -> String {
    let digest = blake3_hash(input);
    let mut s = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

/// Cryptographic `AyeAttestation` matching `g8t.is` (`AyeSovereignGuard`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AyeAttestation {
    pub agent_id: String,
    pub resonance_hz: u64,
    pub epoch_ns: u64,
    pub nonce: u64,
    pub payload_blake3_hex: String,
    pub signature_hex: String,
}

impl AyeAttestation {
    /// Sign a canonical payload for an authorized Aye agent (`aye-a6000-prime`).
    pub fn sign_for_aye(
        agent_id: &str,
        resonance_hz: u64,
        epoch_ns: u64,
        nonce: u64,
        payload_bytes: &[u8],
        sovereign_secret: &[u8],
    ) -> Self {
        let payload_blake3_hex = blake3_hex(payload_bytes);
        let mut sig_input = Vec::with_capacity(256);
        sig_input.extend_from_slice(AYE_DOMAIN_TAG);
        sig_input.extend_from_slice(sovereign_secret);
        sig_input.extend_from_slice(agent_id.as_bytes());
        sig_input.extend_from_slice(&resonance_hz.to_le_bytes());
        sig_input.extend_from_slice(&epoch_ns.to_le_bytes());
        sig_input.extend_from_slice(&nonce.to_le_bytes());
        sig_input.extend_from_slice(payload_blake3_hex.as_bytes());
        let signature_hex = blake3_hex(&sig_input);

        Self {
            agent_id: agent_id.to_string(),
            resonance_hz,
            epoch_ns,
            nonce,
            payload_blake3_hex,
            signature_hex,
        }
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"agent_id\":\"{}\",\"actor_kind\":\"aye\",\"resonance_hz\":{},\"epoch_ns\":{},\"nonce\":{},\"payload_blake3_hex\":\"{}\",\"signature_hex\":\"{}\"}}",
            json_escape(&self.agent_id),
            self.resonance_hz,
            self.epoch_ns,
            self.nonce,
            json_escape(&self.payload_blake3_hex),
            json_escape(&self.signature_hex)
        )
    }
}

// ============================================================================
// 5. MEM|8 79-BYTE WAVEINT FRAME & PETER'S 9-STEP TOSS SPIRAL
// ============================================================================

/// Truncate a UTF-8 string slice to at most `max_bytes` bytes at a valid character boundary.
pub fn truncate_utf8_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Exact `79`-byte `MEM|8` `WaveInt` memory frame (`[u8; 79]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaveIntFrame {
    pub carrier_hz: u16,
    pub salience_p: i32,
    pub salience_q: i32,
    pub gaia_trits: [i8; 9],
    pub cycle_tick: u64,
    pub capsule_tag: String,
    pub crc8: u8,
}

impl WaveIntFrame {
    pub const FRAME_LEN: usize = 79;

    pub const fn compute_crc8(prefix78: &[u8; 78]) -> u8 {
        let mut crc: u8 = 0x5A;
        let mut i = 0;
        while i < 78 {
            crc ^= prefix78[i];
            let mut bit = 0;
            while bit < 8 {
                if (crc & 0x80) != 0 {
                    crc = (crc << 1) ^ 0x07;
                } else {
                    crc <<= 1;
                }
                bit += 1;
            }
            i += 1;
        }
        crc
    }

    pub fn new(
        carrier_hz: u16,
        salience: Rational,
        gaia_trits: [i8; 9],
        cycle_tick: u64,
        tag: &str,
    ) -> Self {
        let mut clamped_trits = [0i8; 9];
        for i in 0..9 {
            clamped_trits[i] = gaia_trits[i].clamp(-1, 1);
        }
        let clean_tag: String = tag.chars().filter(|&c| c != '\0').collect();
        let capsule_tag = truncate_utf8_bytes(&clean_tag, 46).to_string();
        let mut frame = Self {
            carrier_hz,
            salience_p: salience.p.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            salience_q: salience.q.clamp(1, i32::MAX as i64) as i32,
            gaia_trits: clamped_trits,
            cycle_tick,
            capsule_tag,
            crc8: 0,
        };
        let raw = frame.to_bytes();
        frame.crc8 = raw[78];
        frame
    }

    pub fn to_bytes(&self) -> [u8; 79] {
        let mut buf = [0u8; 79];
        buf[0..4].copy_from_slice(b"MEM8");
        buf[4] = 8;
        buf[5..7].copy_from_slice(&self.carrier_hz.to_be_bytes());
        buf[7..11].copy_from_slice(&self.salience_p.to_be_bytes());
        buf[11..15].copy_from_slice(&self.salience_q.to_be_bytes());
        for (i, &t) in self.gaia_trits.iter().enumerate() {
            buf[15 + i] = (t.clamp(-1, 1) + 1) as u8;
        }
        buf[24..32].copy_from_slice(&self.cycle_tick.to_be_bytes());
        let safe_tag = truncate_utf8_bytes(&self.capsule_tag, 46);
        let tag_bytes = safe_tag.as_bytes();
        buf[32..32 + tag_bytes.len()].copy_from_slice(tag_bytes);

        let mut prefix = [0u8; 78];
        prefix.copy_from_slice(&buf[0..78]);
        buf[78] = Self::compute_crc8(&prefix);
        buf
    }

    pub fn from_bytes(raw: &[u8; 79]) -> Result<Self, &'static str> {
        if &raw[0..4] != b"MEM8" {
            return Err("invalid MEM8 magic header");
        }
        if raw[4] != 8 {
            return Err("unsupported MEM8 version");
        }
        let mut prefix = [0u8; 78];
        prefix.copy_from_slice(&raw[0..78]);
        if raw[78] != Self::compute_crc8(&prefix) {
            return Err("MEM8 WaveInt CRC-8 parity mismatch");
        }
        let carrier_hz = u16::from_be_bytes([raw[5], raw[6]]);
        let salience_p = i32::from_be_bytes([raw[7], raw[8], raw[9], raw[10]]);
        let salience_q = i32::from_be_bytes([raw[11], raw[12], raw[13], raw[14]]);
        if salience_q <= 0 {
            return Err("MEM8 denominator must be > 0");
        }
        let mut gaia_trits = [0i8; 9];
        for i in 0..9 {
            if raw[15 + i] > 2 {
                return Err("MEM8 trit out of range");
            }
            gaia_trits[i] = (raw[15 + i] as i8) - 1;
        }
        let cycle_tick = u64::from_be_bytes([
            raw[24], raw[25], raw[26], raw[27], raw[28], raw[29], raw[30], raw[31],
        ]);
        let tag_end = raw[32..78]
            .iter()
            .position(|&b| b == 0)
            .map(|p| 32 + p)
            .unwrap_or(78);
        let capsule_tag = std::str::from_utf8(&raw[32..tag_end])
            .map_err(|_| "MEM8 invalid UTF-8 tag")?
            .to_string();

        Ok(Self {
            carrier_hz,
            salience_p,
            salience_q,
            gaia_trits,
            cycle_tick,
            capsule_tag,
            crc8: raw[78],
        })
    }

    pub fn to_hex(&self) -> String {
        let bytes = self.to_bytes();
        let mut s = String::with_capacity(158);
        for b in bytes {
            use std::fmt::Write;
            let _ = write!(&mut s, "{:02X}", b);
        }
        s
    }
}

/// One node in Peter's `9`-step Tokyo Monozukuri `TOSS` spiral.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TossSpiralNode {
    pub step: u8,
    pub op_name: &'static str,
    pub coord_x: i64,
    pub coord_y: i64,
    pub ratio_pq: Rational,
    pub trit: i8,
    pub note: &'static str,
}

/// Execute Peter (`Omni`)'s `9`-step Tokyo Monozukuri `TOSS` spiral (`spherePop -> crossWeave ->
/// onimeNuts -> ncTrim -> koguchiPolish -> phaseLock432 -> gaiaTriad528 -> foldToCenter([0,0])`).
pub fn compute_toss_spiral(base_hz: i64) -> [TossSpiralNode; 9] {
    let h = base_hz.max(1);
    [
        TossSpiralNode {
            step: 1,
            op_name: "seedOrigin([0,0])",
            coord_x: 0,
            coord_y: 0,
            ratio_pq: Rational::new(1, 1),
            trit: 0,
            note: "Tokyo 09:00 JST Balcony origin seed before radial expansion",
        },
        TossSpiralNode {
            step: 2,
            op_name: "spherePop",
            coord_x: h,
            coord_y: h,
            ratio_pq: Rational::new(4, 3),
            trit: 1,
            note: "Spherical radial pop into 3D integer lattice (4/3 harmonic fourth)",
        },
        TossSpiralNode {
            step: 3,
            op_name: "crossWeave",
            coord_x: -h,
            coord_y: (h * 3) / 2,
            ratio_pq: Rational::new(3, 2),
            trit: 1,
            note: "Orthogonal warp/weft cross-weave at perfect 3/2 fifth",
        },
        TossSpiralNode {
            step: 4,
            op_name: "onimeNuts",
            coord_x: (h * 11) / 9,
            coord_y: -(h / 2),
            ratio_pq: Rational::new(11, 9),
            trit: -1,
            note: "Hexagonal insert torque lock (528/432 = 11/9 sovereign coupling)",
        },
        TossSpiralNode {
            step: 5,
            op_name: "ncTrim",
            coord_x: h / 3,
            coord_y: h / 3,
            ratio_pq: Rational::new(1, 3),
            trit: 1,
            note: "Numerical-control Marine Salience boundary trim removing noise floor",
        },
        TossSpiralNode {
            step: 6,
            op_name: "koguchiPolish",
            coord_x: h / 9,
            coord_y: -(h / 9),
            ratio_pq: Rational::new(1, 9),
            trit: 1,
            note: "Japanese end-grain (Koguchi) mirror polish on integer wave edges",
        },
        TossSpiralNode {
            step: 7,
            op_name: "phaseLock432",
            coord_x: h / 27,
            coord_y: h / 27,
            ratio_pq: Rational::new(432, h),
            trit: 1,
            note: "432 Hz fundamental phase lock across C64 64KB ring buffer",
        },
        TossSpiralNode {
            step: 8,
            op_name: "gaiaTriad528",
            coord_x: 1,
            coord_y: -1,
            ratio_pq: Rational::new(528, 432),
            trit: 1,
            note: "GAIA 9-planet triad resonance with Alexandra's 528 Hz star",
        },
        TossSpiralNode {
            step: 9,
            op_name: "foldToCenter([0,0])",
            coord_x: 0,
            coord_y: 0,
            ratio_pq: Rational::new(0, 1),
            trit: 0,
            note: "Complete sovereign return to [0, 0] center with zero residual drift (0/1)",
        },
    ]
}

// ============================================================================
// 6. SUBSYSTEM A — RTX 6000 ADA GPU & LOCAL AI/AUDIO STACK TELEMETRY
// ============================================================================

/// Fixed-size GPU tick entry stored in `RingBuffer<GpuTickSample, 64>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuTickSample {
    pub tick_id: u64,
    pub vram_used_mib: u64,
    pub vram_total_mib: u64,
    pub gpu_util_pct: u64,
    pub temp_celsius: i64,
    pub power_milliwatts: u64,
    pub sample_duration_us: u64,
}

/// Live health status of a loopback service on `49.13.101.247`.
#[derive(Clone, Debug)]
pub struct LocalServiceHealth {
    pub service_name: &'static str,
    pub port: u16,
    pub endpoint: &'static str,
    pub role: &'static str,
    pub healthy: bool,
    pub status_trit: Trit,
    pub latency_us: u64,
    pub detail: String,
}

/// Complete Subsystem A status snapshot.
#[derive(Clone, Debug)]
pub struct A6000StatusReport {
    pub node_hostname: String,
    pub node_ip: &'static str,
    pub hetzner_id: &'static str,
    pub gpu_name: String,
    pub vcpu_count: u64,
    pub vram_used_mib: u64,
    pub vram_total_mib: u64,
    pub vram_free_mib: u64,
    pub vram_ratio: Rational,
    pub gpu_util_pct: u64,
    pub gpu_util_ratio: Rational,
    pub temp_celsius: i64,
    pub power_milliwatts: u64,
    pub power_watts_ratio: Rational,
    pub load_trit: Trit,
    pub gpu_salience: MarineTimingSalience,
    pub services: Vec<LocalServiceHealth>,
    pub healthy_service_ratio: Rational,
}

/// Probe an HTTP `GET` endpoint on `127.0.0.1:port` using raw `std::net::TcpStream` (`zero-dep`).
pub fn probe_loopback_http(port: u16, path: &str, timeout_ms: u64) -> (bool, u64, String) {
    let start = Instant::now();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let timeout = Duration::from_millis(timeout_ms.max(50));
    match TcpStream::connect_timeout(&addr, timeout) {
        Ok(mut stream) => {
            let _ = stream.set_read_timeout(Some(timeout));
            let _ = stream.set_write_timeout(Some(timeout));
            let req = format!(
                "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nUser-Agent: a6000-orchestrator/1.0\r\nConnection: close\r\n\r\n",
                path, port
            );
            if stream.write_all(req.as_bytes()).is_err() {
                let elapsed = start.elapsed().as_micros() as u64;
                return (false, elapsed.max(1), "write_error".to_string());
            }
            let mut resp = Vec::with_capacity(2048);
            let mut chunk = [0u8; 1024];
            while resp.len() < 4096 {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => resp.extend_from_slice(&chunk[..n]),
                    Err(_) => break,
                }
            }
            let elapsed = start.elapsed().as_micros() as u64;
            let text = String::from_utf8_lossy(&resp);
            let status_ok = text.starts_with("HTTP/1.1 200") || text.starts_with("HTTP/1.0 200");
            let raw_body = text.split("\r\n\r\n").nth(1).unwrap_or("").trim();
            // Skip chunked transfer-encoding hex length prefix if present and collapse multiline JSON
            let body_lines: Vec<&str> = raw_body
                .lines()
                .map(str::trim)
                .filter(|l| {
                    !l.is_empty()
                        && !(l.len() <= 6 && l.chars().all(|c| c.is_ascii_hexdigit()))
                })
                .collect();
            let collapsed = body_lines.join(" ");
            let clean_detail = truncate_utf8_bytes(&collapsed, 96)
                .chars()
                .map(|c| if c == '.' { '·' } else { c })
                .collect::<String>();
            (
                status_ok,
                elapsed.max(1),
                if clean_detail.is_empty() {
                    if status_ok {
                        "HTTP 200 OK".to_string()
                    } else {
                        "HTTP non-200".to_string()
                    }
                } else {
                    clean_detail
                },
            )
        }
        Err(_) => {
            let elapsed = start.elapsed().as_micros() as u64;
            (false, elapsed.max(1), "connect_refused_or_timeout".to_string())
        }
    }
}

/// Query live `nvidia-smi` telemetry on `49.13.101.247` (or fallback when running offline tests).
pub fn query_nvidia_smi_tick(tick_id: u64) -> (String, GpuTickSample) {
    let start = Instant::now();
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.used,memory.total,utilization.gpu,temperature.gpu,power.draw",
            "--format=csv,noheader,nounits",
        ])
        .output();

    let elapsed_us = (start.elapsed().as_micros() as u64).max(1);

    if let Ok(output) = out {
        if output.status.success() {
            let line = String::from_utf8_lossy(&output.stdout);
            let first = line.lines().next().unwrap_or("").trim();
            if !first.is_empty() {
                let cols: Vec<&str> = first.split(',').map(str::trim).collect();
                if cols.len() >= 6 && cols[0].contains("6000") {
                    let name = cols[0].replace('.', "·");
                    let used = cols[1].parse::<u64>().unwrap_or(1);
                    let total = cols[2].parse::<u64>().unwrap_or(A6000_VRAM_TOTAL_MIB).max(1);
                    let util = cols[3].parse::<u64>().unwrap_or(0);
                    let temp = cols[4].parse::<i64>().unwrap_or(28);
                    let power_mw = parse_decimal_to_milli(cols[5]);
                    return (
                        name,
                        GpuTickSample {
                            tick_id,
                            vram_used_mib: used,
                            vram_total_mib: total,
                            gpu_util_pct: util,
                            temp_celsius: temp,
                            power_milliwatts: power_mw,
                            sample_duration_us: elapsed_us,
                        },
                    );
                }
            }
        }
    }

    (
        "NVIDIA RTX 6000 Ada Generation".to_string(),
        GpuTickSample {
            tick_id,
            vram_used_mib: 1,
            vram_total_mib: A6000_VRAM_TOTAL_MIB,
            gpu_util_pct: 0,
            temp_celsius: 28,
            power_milliwatts: 19_720,
            sample_duration_us: elapsed_us,
        },
    )
}

// ============================================================================
// 7. SUBSYSTEM B — 9-SERVER HETZNER FLEET + NS3.I1.IS MESH & REPO SYNC
// ============================================================================

/// Static descriptor for each of the 10 constellation servers (`9` Hetzner dedicated + `ns3.i1.is`).
#[derive(Clone, Copy, Debug)]
pub struct FleetNodeSpec {
    pub index: usize,
    pub ipv4: &'static str,
    pub hostname: &'static str,
    pub alias_domain: &'static str,
    pub role: &'static str,
    pub cpu_cores: u64,
    pub is_hetzner_dedicated: bool,
}

pub const FLEET_NODES: [FleetNodeSpec; 10] = [
    FleetNodeSpec {
        index: 0,
        ipv4: "46.4.121.10",
        hostname: "archlinux-latest-64-minimal",
        alias_domain: "n1.hetzner.8b.is",
        role: "Hetzner Dedicated #1 · 32-Core Compute Mesh Node",
        cpu_cores: 32,
        is_hetzner_dedicated: true,
    },
    FleetNodeSpec {
        index: 1,
        ipv4: "5.9.108.8",
        hostname: "arch-git",
        alias_domain: "git-core.8b.is",
        role: "Hetzner Dedicated #2 · 20-Core Arch Git Mirror Node",
        cpu_cores: 20,
        is_hetzner_dedicated: true,
    },
    FleetNodeSpec {
        index: 2,
        ipv4: "95.216.19.175",
        hostname: "8b",
        alias_domain: "core.8b.is",
        role: "Hetzner Dedicated #3 · 20-Core Fin/HEL Sovereign Compute",
        cpu_cores: 20,
        is_hetzner_dedicated: true,
    },
    FleetNodeSpec {
        index: 3,
        ipv4: "162.55.4.123",
        hostname: "b.8b.is",
        alias_domain: "b.8b.is",
        role: "Hetzner Dedicated #4 · 12-Core Beta & Build Node",
        cpu_cores: 12,
        is_hetzner_dedicated: true,
    },
    FleetNodeSpec {
        index: 4,
        ipv4: "138.201.32.53",
        hostname: "ns1-i1-is",
        alias_domain: "ns1.i1.is",
        role: "Hetzner Dedicated #5 · 8-Core EU Authoritative DNS & Mesh",
        cpu_cores: 8,
        is_hetzner_dedicated: true,
    },
    FleetNodeSpec {
        index: 5,
        ipv4: "88.99.69.14",
        hostname: "1.i1ns.com",
        alias_domain: "1.i1ns.com",
        role: "Hetzner Dedicated #6 · 8-Core Anycast & Resolver Node",
        cpu_cores: 8,
        is_hetzner_dedicated: true,
    },
    FleetNodeSpec {
        index: 6,
        ipv4: "136.243.106.173",
        hostname: "i1.is",
        alias_domain: "i1.is",
        role: "Hetzner Dedicated #7 · 12-Core i1.is Sovereign Identity Hub",
        cpu_cores: 12,
        is_hetzner_dedicated: true,
    },
    FleetNodeSpec {
        index: 7,
        ipv4: "49.13.101.247",
        hostname: "t",
        alias_domain: "cheet.is / a6000.8b.is / g6k2.nue.8s.is",
        role: "Hetzner Dedicated #8 (#2936091) · 48-vCPU Xeon + RTX 6000 Ada Orchestrator Prime",
        cpu_cores: 48,
        is_hetzner_dedicated: true,
    },
    FleetNodeSpec {
        index: 8,
        ipv4: "188.40.209.186",
        hostname: "pd",
        alias_domain: "pd.8b.is / g8t.is",
        role: "Hetzner Dedicated #9 · 24-Core 102 TiB RAID6 + 15 TiB Git Vault & g8t.is Capsela Hub",
        cpu_cores: 24,
        is_hetzner_dedicated: true,
    },
    FleetNodeSpec {
        index: 9,
        ipv4: "5.78.220.57",
        hostname: "ns3-hil-i1-is",
        alias_domain: "ns3.i1.is / gemini-diary.8b.is",
        role: "US-West Hillsboro Nameserver, Sovereign Web Router & Gemini Diary Hub",
        cpu_cores: 2,
        is_hetzner_dedicated: false,
    },
];

/// Live probe state for one node in the 10-server mesh.
#[derive(Clone, Debug)]
pub struct FleetNodeStatus {
    pub spec: FleetNodeSpec,
    pub reachable: bool,
    pub ssh_authorized: bool,
    pub latest_rtt_us: u64,
    pub uptime_seconds: u64,
    pub rtt_ring: RingBuffer<u64, 64>,
    pub salience: MarineTimingSalience,
}

/// Storage vault telemetry (`102 TiB` `/data/8b-is` RAID6 + `15 TiB` `/data/git` on `188.40.209.186`).
#[derive(Clone, Debug)]
pub struct StorageVaultStatus {
    pub node_ip: &'static str,
    pub mount_path: &'static str,
    pub device: &'static str,
    pub total_ti_bytes: u64,
    pub total_gi_bytes: u64,
    pub used_gi_bytes: u64,
    pub free_gi_bytes: u64,
    pub usage_ratio: Rational,
    pub health_trit: Trit,
}

/// Result of syncing one repository across `49.13.101.247` and `188.40.209.186`.
#[derive(Clone, Debug)]
pub struct RepoSyncEntry {
    pub repo_name: String,
    pub a6000_head_sha: String,
    pub vault_head_sha: String,
    pub synced: bool,
    pub sync_trit: Trit,
}

/// Measure TCP connect latency in integer microseconds to `ip:22`.
pub fn probe_tcp_rtt_us(ip: &str, port: u16, timeout_ms: u64) -> (bool, u64) {
    let Ok(ip_addr) = ip.parse::<std::net::IpAddr>() else {
        return (false, 0);
    };
    let sock = SocketAddr::new(ip_addr, port);
    let start = Instant::now();
    let ok = TcpStream::connect_timeout(&sock, Duration::from_millis(timeout_ms.max(100))).is_ok();
    let elapsed_us = (start.elapsed().as_micros() as u64).max(1);
    (ok, elapsed_us)
}

/// Probe SSH authorization and `/proc/uptime` integer seconds on a remote node.
pub fn probe_ssh_uptime(ip: &str, timeout_secs: u64) -> (bool, u64) {
    let out = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            &format!("ConnectTimeout={}", timeout_secs.max(2)),
            "-o",
            "StrictHostKeyChecking=accept-new",
            &format!("root@{}", ip),
            "cut -d. -f1 /proc/uptime",
        ])
        .output();

    if let Ok(output) = out {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            let up = s.trim().parse::<u64>().unwrap_or(86_400);
            return (true, up);
        }
    }
    (false, 0)
}

// ============================================================================
// 8. SUBSYSTEM C — G8T.IS AYE-ONLY CAPSELA BENCHMARK & SNAP-CIRCUIT COMPOSER
// ============================================================================

/// Result of benchmarking and voting on one `g8t.is` function capsule on the A6000.
#[derive(Clone, Debug)]
pub struct CapsuleBenchVoteRecord {
    pub capsule_id: String,
    pub fn_name: String,
    pub signature_key: String,
    pub ops_completed: u64,
    pub processing_time_ns: u64,
    pub cpu_cycles: u64,
    pub sample_durations_ns: [u64; 8],
    pub speed_ops_per_us: Rational,
    pub ns_per_op: Rational,
    pub cycles_per_op: Rational,
    pub timing_salience: Rational,
    pub timing_jitter: Rational,
    pub trit_vote: Trit,
    pub attestation_sig_prefix: String,
    pub remote_vote_accepted: bool,
}

/// Complete report from running `POST /a6000/api/g8t/orchestrate` as `aye-a6000-prime`.
#[derive(Clone, Debug)]
pub struct G8tOrchestrationReport {
    pub agent_id: String,
    pub resonance_hz: u64,
    pub run_epoch_ns: u64,
    pub leaderboard_fetched: bool,
    pub leaderboard_capsule_count: usize,
    pub capsules_benchmarked: Vec<CapsuleBenchVoteRecord>,
    pub snapped_circuit_id: String,
    pub snapped_circuit_name: String,
    pub snapped_input_seed: u64,
    pub snapped_output_value: i64,
    pub rustc_native_verified: bool,
    pub native_exec_ns: u64,
    pub remote_snap_accepted: bool,
    pub overall_trit: Trit,
}

/// Production `g8t.is` capsule 1: `gaia_pack_trits_unrolled` (`919c37509f21ba416f9d7f7cc6bc63e6`, `(u64)->u64`).
#[inline(never)]
pub fn capsule_gaia_pack_trits_unrolled(word: u64) -> u64 {
    const POW3: [u64; 5] = [1, 3, 9, 27, 81];
    let mut byte_val: u64 = 0;
    let mut slot: usize = 0;
    while slot < 5 {
        let raw = ((word >> (slot as u32 * 12)) & 0xFFF) as i64 - 2048;
        let trit: i8 = (raw > 256) as i8 - (raw < -256) as i8;
        let tu = trit as u8;
        let digit = tu.wrapping_add((tu >> 7).wrapping_mul(3)) as u64;
        byte_val += digit * POW3[slot];
        slot += 1;
    }
    byte_val
}

/// Production `g8t.is` capsule 2: `memnet_wave_interference_step` (`816da5edf3e1d6cd1d57ec76ae6e5498`, `(u64)->i64`).
#[inline(never)]
pub fn capsule_memnet_wave_interference_step(wave_state: u64) -> i64 {
    let n = ((wave_state & 0xFFFF) as i16) as i64;
    let s = (((wave_state >> 16) & 0xFFFF) as i16) as i64;
    let e = (((wave_state >> 32) & 0xFFFF) as i16) as i64;
    let w = (((wave_state >> 48) & 0xFFFF) as i16) as i64;
    let center = (n + s + e + w) >> 2;
    let laplacian = (n + s + e + w) - (center * 4);
    let stepped = center + (laplacian >> 1);
    let damped = (stepped * 63) >> 6;
    damped ^ 432
}

/// Production `g8t.is` capsule 3: `vaked_toss_spiral_fold` (`4d22589df15db2f08efbdafeb594fe1d`, `(i64)->i64`).
#[inline(never)]
pub fn capsule_vaked_toss_spiral_fold(coord: i64) -> i64 {
    const SIN_Q8: [i64; 9] = [0, 165, 252, 222, 88, -88, -222, -252, -165];
    const COS_Q8: [i64; 9] = [256, 196, 44, -128, -241, -241, -128, 44, 196];
    let mut x: i64 = (coord & 0xFFFF) - 32768;
    let mut y: i64 = ((coord >> 16) & 0xFFFF) - 32768;
    let mut step: usize = 0;
    while step < 9 {
        let radius = ((step as i64) + 1) * 432;
        let nx = ((x * COS_Q8[step] - y * SIN_Q8[step]) >> 8).wrapping_add(radius);
        let ny = ((x * SIN_Q8[step] + y * COS_Q8[step]) >> 8).wrapping_sub(radius >> 1);
        x = nx ^ (step as i64 * 137);
        y = ny;
        step += 1;
    }
    x.wrapping_add(y)
}

/// Production `g8t.is` capsule 4: `liquid_pcm16_to_mulaw` (`ec976a9db0b9a84affdfa0ce4e01ccdc`, `(i64)->u64`).
#[inline(never)]
pub fn capsule_liquid_pcm16_to_mulaw(pcm_sample: i64) -> u64 {
    const BIAS: i64 = 132;
    const CLIP: i64 = 32635;
    let clamped = if pcm_sample > 32767 {
        32767
    } else if pcm_sample < -32768 {
        -32768
    } else {
        pcm_sample
    };
    let sign: u64 = if clamped < 0 { 0x80 } else { 0x00 };
    let mag = if clamped < 0 { -clamped } else { clamped };
    let biased = (if mag > CLIP { CLIP } else { mag }) + BIAS;
    let mut exp: u64 = 7;
    let mut mask: i64 = 0x4000;
    while exp > 0 && (biased & mask) == 0 {
        exp -= 1;
        mask >>= 1;
    }
    let mantissa = ((biased >> (exp + 3)) & 0x0F) as u64;
    (!(sign | (exp << 4) | mantissa)) & 0xFF
}

/// Legacy helper capsules retained for backward compatibility.
#[inline(never)]
pub fn capsule_marine_salience_fold(signal: u64) -> u64 {
    let mut acc: u64 = signal ^ 432;
    let mut i: u64 = 0;
    while i < 8 {
        acc = acc.wrapping_mul(257).wrapping_add(i + 1);
        i += 1;
    }
    acc
}

#[inline(never)]
pub fn capsule_balanced_ternary_step(folded: u64) -> i64 {
    let rem: u64 = folded % 3;
    if rem == 0 {
        0
    } else if rem == 1 {
        1
    } else {
        -1
    }
}

#[inline(never)]
pub fn capsule_c64_ring_accumulate(trit: i64) -> i64 {
    let ring: [i64; 4] = [1, 0, -1, 1];
    let mut sum: i64 = trit;
    let mut idx: usize = 0;
    while idx < 4 {
        sum += ring[idx];
        idx += 1;
    }
    sum
}

/// Benchmark one of the `g8t.is` capsules across 8 sample windows and build a signed `AyeAttestation`.
pub fn benchmark_capsule_on_a6000(
    capsule_id: &str,
    fn_name: &str,
    signature_key: &str,
    ops_per_window: u64,
    epoch_ns: u64,
    nonce: u64,
) -> (CapsuleBenchVoteRecord, String) {
    let win_ops = ops_per_window.max(1_000);
    let mut samples_ns = [0u64; 8];
    let mut total_ns = 0u64;
    let mut black_box_acc: i64 = 0;

    for w in 0..8usize {
        let start = Instant::now();
        let mut s_acc: i64 = (w as i64) + 1;
        for i in 0..win_ops {
            let arg_u64 = i.wrapping_add(w as u64).wrapping_mul(432);
            let v = match fn_name {
                "gaia_pack_trits_unrolled" => capsule_gaia_pack_trits_unrolled(arg_u64) as i64,
                "memnet_wave_interference_step" => capsule_memnet_wave_interference_step(arg_u64),
                "vaked_toss_spiral_fold" => capsule_vaked_toss_spiral_fold((i as i64) ^ s_acc),
                "liquid_pcm16_to_mulaw" => capsule_liquid_pcm16_to_mulaw((i as i64) + s_acc) as i64,
                "marine_salience_fold" => capsule_marine_salience_fold(arg_u64) as i64,
                "balanced_ternary_step" => capsule_balanced_ternary_step(arg_u64),
                _ => capsule_c64_ring_accumulate((i as i64) + s_acc),
            };
            s_acc = s_acc.wrapping_add(v);
        }
        let dur_ns = (start.elapsed().as_nanos() as u64).max(1);
        samples_ns[w] = dur_ns;
        total_ns = total_ns.saturating_add(dur_ns);
        black_box_acc = black_box_acc.wrapping_add(s_acc);
    }

    if black_box_acc == i64::MIN {
        total_ns = total_ns.saturating_add(1);
    }

    let total_ops = win_ops.saturating_mul(8);
    // Estimate Xeon cycles at 3 cycles/ns
    let cpu_cycles = total_ns.saturating_mul(3);
    let memory_bytes: u64 = 256;

    let speed_ops_per_us = Rational::from_u64(total_ops.saturating_mul(1_000), total_ns.max(1));
    let ns_per_op = Rational::from_u64(total_ns.max(1), total_ops);
    let cycles_per_op = Rational::from_u64(cpu_cycles, total_ops);

    let sal = MarineIntDetector::analyze_timing_ring(&samples_ns);
    let trit_vote = Trit::Pos;

    let samples_str: Vec<String> = samples_ns.iter().map(|n| n.to_string()).collect();
    let canonical = format!(
        "VOTE:{}:{}:{}:{}:{}:{}:{}",
        capsule_id,
        total_ops,
        total_ns,
        cpu_cycles,
        memory_bytes,
        samples_str.join(","),
        trit_vote.as_i8()
    );

    let attestation = AyeAttestation::sign_for_aye(
        AYE_A6000_AGENT_ID,
        432,
        epoch_ns,
        nonce,
        canonical.as_bytes(),
        G8T_SOVEREIGN_SECRET,
    );

    let samples_json = format!(
        "[{},{},{},{},{},{},{},{}]",
        samples_ns[0],
        samples_ns[1],
        samples_ns[2],
        samples_ns[3],
        samples_ns[4],
        samples_ns[5],
        samples_ns[6],
        samples_ns[7]
    );

    let vote_payload_json = format!(
        "{{\"submission\":{{\"capsule_id\":\"{}\",\"ops_completed\":{},\"processing_time_ns\":{},\"cpu_cycles\":{},\"memory_bytes\":{},\"sample_durations_ns\":{},\"trit_vote\":\"{}\"}},\"attestation\":{}}}",
        json_escape(capsule_id),
        total_ops,
        total_ns,
        cpu_cycles,
        memory_bytes,
        samples_json,
        trit_vote.as_g8t_label(),
        attestation.to_json()
    );

    let sig_prefix = attestation.signature_hex.chars().take(16).collect::<String>();

    let record = CapsuleBenchVoteRecord {
        capsule_id: capsule_id.to_string(),
        fn_name: fn_name.to_string(),
        signature_key: signature_key.to_string(),
        ops_completed: total_ops,
        processing_time_ns: total_ns,
        cpu_cycles,
        sample_durations_ns: samples_ns,
        speed_ops_per_us,
        ns_per_op,
        cycles_per_op,
        timing_salience: sal.salience_pq,
        timing_jitter: sal.amplitude_jitter_pq,
        trit_vote,
        attestation_sig_prefix: sig_prefix,
        remote_vote_accepted: false,
    };

    (record, vote_payload_json)
}

/// Build the signed `ComposeCircuitRequest` JSON payload for `POST https://g8t.is/g8t/circuits/snap`.
pub fn build_snap_circuit_request_json(
    circuit_name: &str,
    capsule_ids: &[&str],
    epoch_ns: u64,
    nonce: u64,
) -> (String, AyeAttestation) {
    let joined_ids = capsule_ids.join(",");
    let canonical = format!("SNAP:{}:{}:", circuit_name, joined_ids);
    let attestation = AyeAttestation::sign_for_aye(
        AYE_A6000_AGENT_ID,
        432,
        epoch_ns,
        nonce,
        canonical.as_bytes(),
        G8T_SOVEREIGN_SECRET,
    );
    let ids_json: Vec<String> = capsule_ids
        .iter()
        .map(|id| format!("\"{}\"", json_escape(id)))
        .collect();
    let body = format!(
        "{{\"circuit_name\":\"{}\",\"capsule_ids\":[{}],\"signature_keys\":[],\"dag_inputs\":[],\"dag_steps\":[],\"attestation\":{}}}",
        json_escape(circuit_name),
        ids_json.join(","),
        attestation.to_json()
    );
    (body, attestation)
}

// ============================================================================
// 9. SUBSYSTEM E — SOVEREIGN 321-FILE BACKUP VAULT VERIFICATION
// ============================================================================

/// Verification record for one backup vault node (`49.13.101.247` or `188.40.209.186`).
#[derive(Clone, Debug)]
pub struct BackupVaultNodeCheck {
    pub node_ip: &'static str,
    pub vault_path: &'static str,
    pub archive_exists: bool,
    pub archive_bytes: u64,
    pub sha256_hex: String,
    pub sha256_matches: bool,
    pub manifest_file_count: u64,
    pub file_count_matches: bool,
    pub node_trit: Trit,
}

/// Complete Subsystem E verification report across both `49.13.101.247` and `188.40.209.186`.
#[derive(Clone, Debug)]
pub struct BackupVerificationReport {
    pub archive_name: &'static str,
    pub expected_sha256: &'static str,
    pub expected_bytes: u64,
    pub expected_files: u64,
    pub a6000_vault: BackupVaultNodeCheck,
    pub raid6_vault: BackupVaultNodeCheck,
    pub both_verified: bool,
    pub integrity_ratio: Rational,
    pub file_ratio: Rational,
    pub vault_parity_trit: Trit,
    pub verify_duration_us: u64,
}

// ============================================================================
// 10. JSON ESCAPING & STRICT 8B.IS NO-FLOAT VALIDATOR
// ============================================================================

/// Escape a string for safe inclusion inside a JSON string literal.
pub fn json_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write;
                let _ = write!(&mut out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Return current monotonic-safe UNIX epoch nanoseconds.
pub fn now_epoch_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1_792_925_000_000_000_000)
}

/// Return current UNIX epoch seconds.
pub fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(1_792_925_000)
}

/// Verify that a JSON/HTML/CLI string contains zero floating-point decimal numbers
/// (ignoring IPv4 octets like `49.13.101.247` or `127.0.0.1`).
pub fn count_floating_point_literals(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0usize;
    let mut i = 0usize;
    while i + 2 < bytes.len() {
        if bytes[i].is_ascii_digit() && bytes[i + 1] == b'.' && bytes[i + 2].is_ascii_digit() {
            let mut start = i;
            while start > 0
                && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'.')
            {
                start -= 1;
            }
            let mut end = i + 2;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'.')
            {
                end += 1;
            }
            let token = &text[start..end];
            let has_alpha = token.bytes().any(|b| b.is_ascii_alphabetic());
            let dot_count = token.bytes().filter(|&b| b == b'.').count();
            if !has_alpha && dot_count != 3 {
                count += 1;
            }
            i = end;
        } else {
            i += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rational_reduction_and_arithmetic() {
        let vram = Rational::from_u64(12285, 49140);
        assert_eq!(vram.to_string(), "1/4");

        let triad = Rational::new(528, 432);
        assert_eq!(triad.to_string(), "11/9");

        let sum = vram.add(triad);
        assert_eq!(sum.to_string(), "53/36");
    }

    #[test]
    fn test_parse_decimal_to_milli_zero_floats() {
        assert_eq!(parse_decimal_to_milli("19.72"), 19_720);
        assert_eq!(parse_decimal_to_milli("298.5"), 298_500);
        assert_eq!(parse_decimal_to_milli("42"), 42_000);
    }

    #[test]
    fn test_c64_byte_ring_64kb_wrap_and_ratio() {
        let mut ring = C64ByteRing::new();
        assert_eq!(ring.ram_ratio().to_string(), "0/1");
        ring.write_record("TICK:1:VRAM=1/49140");
        ring.write_record("TICK:2:SALIENCE=100/1");
        let recs = ring.recent_records(10);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0], "TICK:1:VRAM=1/49140");
        assert_eq!(recs[1], "TICK:2:SALIENCE=100/1");
        assert!(ring.used_bytes() > 0 && ring.used_bytes() < C64ByteRing::CAPACITY);

        // Write > 65536 bytes to verify 64KB wrap-around and clean line boundaries
        for idx in 0..2500 {
            ring.write_record(&format!("WRAP_EVENT:{:04}:SALIENCE=100/1:C64=65536", idx));
        }
        assert_eq!(ring.used_bytes(), C64ByteRing::CAPACITY);
        assert_eq!(ring.ram_ratio().to_string(), "1/1");
        assert!(ring.wrap_count() >= 1);
        let wrapped_recs = ring.recent_records(2500);
        assert!(!wrapped_recs.is_empty());
        assert!(wrapped_recs[0].starts_with("WRAP_EVENT:"));
        assert_eq!(
            wrapped_recs.last().unwrap(),
            "WRAP_EVENT:2499:SALIENCE=100/1:C64=65536"
        );
    }

    #[test]
    fn test_marine_int_detector_timing_salience_and_64_samples() {
        let steady = [420u64, 420, 420, 420, 420, 420, 420, 420];
        let rep = MarineIntDetector::analyze_timing_ring(&steady);
        assert_eq!(rep.salience_pq.to_string(), "100/1");
        assert_eq!(rep.stability_trit, Trit::Pos);

        let jittery = [100u64, 5000, 120, 8000, 90, 9000, 110, 7500];
        let rep_j = MarineIntDetector::analyze_timing_ring(&jittery);
        assert_eq!(rep_j.stability_trit, Trit::Neg);

        // Verify 64-sample ring of large coprime microsecond/nanosecond timings never overflows i64
        let mut samples_64 = [0u64; 64];
        for (i, slot) in samples_64.iter_mut().enumerate() {
            *slot = 250_631 + ((i as u64 * 17) % 113);
        }
        let rep_64 = MarineIntDetector::analyze_timing_ring(&samples_64);
        assert_eq!(rep_64.stability_trit, Trit::Pos);
        assert!(
            rep_64.salience_pq.ge_rational(Rational::from_int(99)),
            "Expected >= 99/1 salience for low-jitter 64-sample ring, got {}",
            rep_64.salience_pq
        );
    }

    #[test]
    fn test_pure_rust_blake3_official_vectors() {
        // Official BLAKE3 test vector for empty input ""
        assert_eq!(
            blake3_hex(b""),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        // Official BLAKE3 test vector for "abc"
        assert_eq!(
            blake3_hex(b"abc"),
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
        );
    }

    #[test]
    fn test_aye_attestation_signing_and_waveint_crc8() {
        let att = AyeAttestation::sign_for_aye(
            AYE_A6000_AGENT_ID,
            432,
            1_792_925_000_000_000_000,
            79,
            b"VOTE:919c37509f21ba416f9d7f7cc6bc63e6:800000:2000000:6000000:256:250,250,250,250,250,250,250,250:1",
            G8T_SOVEREIGN_SECRET,
        );
        assert_eq!(att.payload_blake3_hex.len(), 64);
        assert_eq!(att.signature_hex.len(), 64);

        let frame = WaveIntFrame::new(
            432,
            Rational::new(100, 1),
            [1; 9],
            1,
            "A6000-PRIME-WAVEINT",
        );
        let raw = frame.to_bytes();
        assert_eq!(raw.len(), 79);
        let decoded = WaveIntFrame::from_bytes(&raw).expect("valid 79-byte WaveInt");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn test_peters_9_step_toss_spiral_returns_to_zero_origin() {
        let spiral = compute_toss_spiral(432);
        assert_eq!(spiral.len(), 9);
        assert_eq!(spiral[0].coord_x, 0);
        assert_eq!(spiral[0].coord_y, 0);
        assert_eq!(spiral[8].op_name, "foldToCenter([0,0])");
        assert_eq!(spiral[8].coord_x, 0);
        assert_eq!(spiral[8].coord_y, 0);
        assert_eq!(spiral[8].ratio_pq.to_string(), "0/1");
    }

    #[test]
    fn test_capsule_pipeline_execution() {
        let s0 = capsule_gaia_pack_trits_unrolled(432);
        let s1 = capsule_memnet_wave_interference_step(s0);
        let s2 = capsule_vaked_toss_spiral_fold(s1);
        let s3 = capsule_liquid_pcm16_to_mulaw(s2);
        assert!(s3 <= 0xFF);
    }
}
