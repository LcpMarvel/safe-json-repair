//! Performance benchmarks for the PRD §3.3 targets:
//!
//!  * **Fast-path overhead ≈ one `serde_json::from_str`.** We bench valid 1 MB
//!    JSON through `repair` *and* through bare `serde_json::from_str` in the
//!    same group, so the ratio is visible directly — the PRD asks that the
//!    repair ladder cost ~nothing when it doesn't fire.
//!  * **1 MB broken input repairs in < 50 ms.** We bench the headline LLM shape
//!    (premature root close + orphaned sibling) scaled to ~1 MB, forcing the
//!    level-5 tolerant parser end to end.
//!  * **Per-call latency on the real-world sample** (corpus `C1-full`).
//!
//! Run with `cargo bench -j 2` (the workspace caps build parallelism). The
//! `bench` profile inherits the speed-tuned `release` profile (`opt-level = 3`,
//! see workspace Cargo.toml), so these reflect native server throughput.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use safe_json_repair::{repair, Options};

/// Build a valid JSON object of roughly `target` bytes: `{"data":[ {..}, .. ]}`.
fn valid_payload(target: usize) -> String {
    let mut s = String::with_capacity(target + 64);
    s.push_str(r#"{"data":["#);
    let mut i = 0usize;
    // Each record is a fixed, realistic-ish object; append until we cross target.
    while s.len() < target {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            r#"{{"id":{i},"name":"item-{i}","value":{},"ok":true}}"#,
            i * 7 % 1000
        ));
        i += 1;
    }
    s.push_str("]}");
    s
}

/// The same payload, broken the LLM-typical way: an extra `]}` closes the root
/// prematurely, then a sibling key is orphaned after it. Only the level-5
/// tolerant parser (the root-sibling heuristic) recovers this without data loss.
fn broken_payload(target: usize) -> String {
    let valid = valid_payload(target);
    // valid ends with "]}". Insert a spurious extra "]}" and an orphaned sibling.
    let body = valid.strip_suffix("]}").expect("valid payload ends with ]}");
    format!(r#"{body}]]}}, "summary":"order-to-cash across four lanes"}}"#)
}

fn real_sample() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/cases.json");
    let raw = std::fs::read_to_string(path).expect("read corpus/cases.json");
    let cases: serde_json::Value = serde_json::from_str(&raw).expect("parse corpus");
    cases
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "C1-full")
        .expect("C1-full present")["input"]
        .as_str()
        .unwrap()
        .to_string()
}

const ONE_MB: usize = 1_000_000;

fn bench_fast_path(c: &mut Criterion) {
    let valid = valid_payload(ONE_MB);
    let opts = Options::default();

    let mut g = c.benchmark_group("fast_path_1mb_valid");
    g.throughput(Throughput::Bytes(valid.len() as u64));
    // Our fast-path: strict parse + verbatim return.
    g.bench_function("repair", |b| {
        b.iter(|| {
            let r = repair(black_box(&valid), black_box(&opts));
            black_box(r.value)
        })
    });
    // Baseline: bare serde_json. The PRD target is that "repair" ≈ this.
    g.bench_function("serde_json_baseline", |b| {
        b.iter(|| {
            let v: serde_json::Value = serde_json::from_str(black_box(&valid)).unwrap();
            black_box(v)
        })
    });
    g.finish();
}

fn bench_broken(c: &mut Criterion) {
    let broken = broken_payload(ONE_MB);
    let opts = Options::default();

    let mut g = c.benchmark_group("tolerant_1mb_broken");
    g.throughput(Throughput::Bytes(broken.len() as u64));
    // Sanity: confirm the bench input actually exercises the tolerant path and
    // recovers the orphaned sibling (a fast-path success would invalidate it).
    let probe = repair(&broken, &opts);
    assert_eq!(probe.strategy, safe_json_repair::Strategy::Tolerant);
    assert!(probe.value.get("summary").is_some(), "sibling must survive");

    g.bench_function("repair", |b| {
        b.iter(|| {
            let r = repair(black_box(&broken), black_box(&opts));
            black_box(r.value)
        })
    });
    g.finish();
}

fn bench_real_sample(c: &mut Criterion) {
    let sample = real_sample();
    let opts = Options::default();
    c.bench_function("real_sample_c1_full", |b| {
        b.iter(|| {
            let r = repair(black_box(&sample), black_box(&opts));
            black_box(r.value)
        })
    });
}

criterion_group!(benches, bench_fast_path, bench_broken, bench_real_sample);
criterion_main!(benches);
