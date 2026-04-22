//! Benchmarks for iddqd.
//!
//! # Workloads
//!
//! * `get/...` — point lookup on a filled map. Swept across a wide
//!   size range because the routine is cheap enough per iteration to
//!   probe cache/scaling effects.
//! * `bulk_insert/...` — insert `N` records into a fresh map.
//! * `churn/...` — pre-fill, then remove + reinsert the same key at
//!   steady state.
//! * `iter/...` — full iteration over a populated map.
//! * `shrink_to_fit/...` — pre-fill, scatter ~50% holes, compact.
//! * `ref_mut/id_ord_map` — `IdOrdMap`'s mutable-reference guard
//!   overhead, isolated from the hash function.

use criterion::{
    BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main,
};
use iddqd::{DefaultHashBuilder, IdHashMap, IdOrdMap};
use iddqd_benches::{RecordBorrowedU32, RecordOwnedU32};
use iddqd_test_utils::test_item::{TestItem, TestKey1};
use std::collections::{BTreeMap, HashMap};

/// Size sweep for `get` benches. The routine is fast enough per
/// iteration to cover several orders of magnitude.
const GET_SIZES: &[usize] =
    &[1, 10, 100, 1_000, 10_000, 50_000, 100_000, 500_000, 1_000_000];

/// Size sweep for the remaining workloads. Each iteration does a full
/// insert / churn / iter / shrink pass, so the range is kept narrow.
/// Chosen to span cache-resident, L2/L3, and main-memory regimes.
const SIZES: &[usize] = &[100, 10_000, 100_000];

/// Number of remove + reinsert pairs per churn iteration.
const CHURN_OPS: usize = 1_000;

fn record(i: u32) -> RecordOwnedU32 {
    RecordOwnedU32 { index: i, data: String::new() }
}

fn record_borrowed(i: u32) -> RecordBorrowedU32 {
    RecordBorrowedU32 { index: i, data: String::new() }
}

// ---------- get ------------------------------------------------------------

/// Sweep `GET_SIZES` for a `(build, get)` pair: on each iteration,
/// rebuild a map of size `N` via `build`, then measure one invocation
/// of `get`. Setup is excluded from the measurement.
fn bench_get<M>(
    c: &mut Criterion,
    name: &str,
    build: impl Fn(usize) -> M,
    get: impl Fn(&M),
) {
    let mut group = c.benchmark_group(name);
    for &size in GET_SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched_ref(
                    || build(size),
                    |m| get(m),
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn get_hash_map(c: &mut Criterion) {
    bench_get(
        c,
        "get/std_hash_map",
        |n| {
            let mut m = HashMap::with_hasher(DefaultHashBuilder::default());
            for i in 0..n as u32 {
                m.insert(i, record(i));
            }
            m
        },
        |m| {
            m.get(&0);
        },
    );
}

fn get_btree_map(c: &mut Criterion) {
    bench_get(
        c,
        "get/std_btree_map",
        |n| {
            let mut m = BTreeMap::new();
            for i in 0..n as u32 {
                m.insert(i, record(i));
            }
            m
        },
        |m| {
            m.get(&0);
        },
    );
}

fn get_id_hash_map_owned(c: &mut Criterion) {
    bench_get(
        c,
        "get/id_hash_map/owned",
        |n| {
            let mut m = IdHashMap::new();
            for i in 0..n as u32 {
                m.insert_overwrite(record(i));
            }
            m
        },
        |m| {
            m.get(&0);
        },
    );
}

fn get_id_hash_map_borrowed(c: &mut Criterion) {
    bench_get(
        c,
        "get/id_hash_map/borrowed",
        |n| {
            let mut m = IdHashMap::new();
            for i in 0..n as u32 {
                m.insert_overwrite(record_borrowed(i));
            }
            m
        },
        |m| {
            m.get(&0);
        },
    );
}

fn get_id_ord_map_owned(c: &mut Criterion) {
    bench_get(
        c,
        "get/id_ord_map/owned",
        |n| {
            let mut m = IdOrdMap::new();
            for i in 0..n as u32 {
                m.insert_overwrite(record(i));
            }
            m
        },
        |m| {
            m.get(&0);
        },
    );
}

fn get_id_ord_map_borrowed(c: &mut Criterion) {
    bench_get(
        c,
        "get/id_ord_map/borrowed",
        |n| {
            let mut m = IdOrdMap::new();
            for i in 0..n as u32 {
                m.insert_overwrite(record_borrowed(i));
            }
            m
        },
        |m| {
            m.get(&0);
        },
    );
}

// ---------- bulk_insert ----------------------------------------------------

fn bulk_insert_std_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_insert/std_hash_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || (),
                    |_| {
                        let mut map =
                            HashMap::with_hasher(DefaultHashBuilder::default());
                        for i in 0..size as u32 {
                            map.insert(i, record(i));
                        }
                        map
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn bulk_insert_std_btree_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_insert/std_btree_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || (),
                    |_| {
                        let mut map = BTreeMap::new();
                        for i in 0..size as u32 {
                            map.insert(i, record(i));
                        }
                        map
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn bulk_insert_id_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_insert/id_hash_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || (),
                    |_| {
                        let mut map = IdHashMap::new();
                        for i in 0..size as u32 {
                            map.insert_unique(record(i)).unwrap();
                        }
                        map
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn bulk_insert_id_ord_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_insert/id_ord_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || (),
                    |_| {
                        let mut map = IdOrdMap::new();
                        for i in 0..size as u32 {
                            map.insert_unique(record(i)).unwrap();
                        }
                        map
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

// ---------- churn ----------------------------------------------------------

/// Churn workload: pre-fill with `size` records, then run `CHURN_OPS`
/// iterations where each iteration removes a key and inserts the same
/// record back.
fn churn_std_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("churn/std_hash_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched_ref(
                    || {
                        let mut map =
                            HashMap::with_hasher(DefaultHashBuilder::default());
                        for i in 0..size as u32 {
                            map.insert(i, record(i));
                        }
                        map
                    },
                    |map| {
                        let size = size as u32;
                        for step in 0..CHURN_OPS as u32 {
                            let key = step % size;
                            let v = map.remove(&key).unwrap();
                            map.insert(key, v);
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn churn_std_btree_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("churn/std_btree_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched_ref(
                    || {
                        let mut map = BTreeMap::new();
                        for i in 0..size as u32 {
                            map.insert(i, record(i));
                        }
                        map
                    },
                    |map| {
                        let size = size as u32;
                        for step in 0..CHURN_OPS as u32 {
                            let key = step % size;
                            let v = map.remove(&key).unwrap();
                            map.insert(key, v);
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn churn_id_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("churn/id_hash_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched_ref(
                    || {
                        let mut map = IdHashMap::new();
                        for i in 0..size as u32 {
                            map.insert_unique(record(i)).unwrap();
                        }
                        map
                    },
                    |map| {
                        let size = size as u32;
                        for step in 0..CHURN_OPS as u32 {
                            let key = step % size;
                            let v = map.remove(&key).unwrap();
                            map.insert_unique(v).unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn churn_id_ord_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("churn/id_ord_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched_ref(
                    || {
                        let mut map = IdOrdMap::new();
                        for i in 0..size as u32 {
                            map.insert_unique(record(i)).unwrap();
                        }
                        map
                    },
                    |map| {
                        let size = size as u32;
                        for step in 0..CHURN_OPS as u32 {
                            let key = step % size;
                            let v = map.remove(&key).unwrap();
                            map.insert_unique(v).unwrap();
                        }
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

// ---------- iter -----------------------------------------------------------

fn iter_std_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter/std_hash_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                let mut map =
                    HashMap::with_hasher(DefaultHashBuilder::default());
                for i in 0..size as u32 {
                    map.insert(i, record(i));
                }
                b.iter(|| {
                    let mut sum: u64 = 0;
                    for r in map.values() {
                        sum = sum.wrapping_add(r.index as u64);
                    }
                    sum
                });
            },
        );
    }
    group.finish();
}

fn iter_std_btree_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter/std_btree_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                let mut map = BTreeMap::new();
                for i in 0..size as u32 {
                    map.insert(i, record(i));
                }
                b.iter(|| {
                    let mut sum: u64 = 0;
                    for r in map.values() {
                        sum = sum.wrapping_add(r.index as u64);
                    }
                    sum
                });
            },
        );
    }
    group.finish();
}

fn iter_id_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter/id_hash_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                let mut map = IdHashMap::new();
                for i in 0..size as u32 {
                    map.insert_unique(record(i)).unwrap();
                }
                b.iter(|| {
                    let mut sum: u64 = 0;
                    for r in &map {
                        sum = sum.wrapping_add(r.index as u64);
                    }
                    sum
                });
            },
        );
    }
    group.finish();
}

fn iter_id_ord_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter/id_ord_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                let mut map = IdOrdMap::new();
                for i in 0..size as u32 {
                    map.insert_unique(record(i)).unwrap();
                }
                b.iter(|| {
                    let mut sum: u64 = 0;
                    for r in &map {
                        sum = sum.wrapping_add(r.index as u64);
                    }
                    sum
                });
            },
        );
    }
    group.finish();
}

// ---------- shrink_to_fit --------------------------------------------------

/// Pre-fill with `size` records, remove every other key to scatter
/// ~50% holes, then shrink.
///
/// `BTreeMap` has no `shrink_to_fit`, so only `HashMap` is included
/// among the std comparators.
fn shrink_to_fit_std_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("shrink_to_fit/std_hash_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut map =
                            HashMap::with_hasher(DefaultHashBuilder::default());
                        for i in 0..size as u32 {
                            map.insert(i, record(i));
                        }
                        for i in (0..size as u32).step_by(2) {
                            map.remove(&i).unwrap();
                        }
                        map
                    },
                    |mut map| {
                        map.shrink_to_fit();
                        map
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn shrink_to_fit_id_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("shrink_to_fit/id_hash_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut map = IdHashMap::new();
                        for i in 0..size as u32 {
                            map.insert_unique(record(i)).unwrap();
                        }
                        for i in (0..size as u32).step_by(2) {
                            map.remove(&i).unwrap();
                        }
                        map
                    },
                    |mut map| {
                        map.shrink_to_fit();
                        map
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn shrink_to_fit_id_ord_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("shrink_to_fit/id_ord_map");
    for &size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut map = IdOrdMap::new();
                        for i in 0..size as u32 {
                            map.insert_unique(record(i)).unwrap();
                        }
                        for i in (0..size as u32).step_by(2) {
                            map.remove(&i).unwrap();
                        }
                        map
                    },
                    |mut map| {
                        map.shrink_to_fit();
                        map
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

// ---------- ref_mut --------------------------------------------------------

/// Benchmarks the overhead of `IdOrdMap::get_mut`'s `RefMut` guard
/// with a trivial hash function, so the measurement isolates the
/// guard cost from the hasher.
fn ref_mut_id_ord_map(c: &mut Criterion) {
    c.bench_function("ref_mut/id_ord_map", |b| {
        b.iter_batched_ref(
            || {
                let mut map = IdOrdMap::new();
                map.insert_overwrite(TestItem::new(1, 'a', "foo", "bar"));
                map
            },
            |map| {
                let mut item = map.get_mut(&TestKey1::new(&1)).unwrap();
                item.key2 = 'b';
                drop(item);
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    get_hash_map,
    get_btree_map,
    get_id_hash_map_owned,
    get_id_hash_map_borrowed,
    get_id_ord_map_owned,
    get_id_ord_map_borrowed,
    bulk_insert_std_hash_map,
    bulk_insert_std_btree_map,
    bulk_insert_id_hash_map,
    bulk_insert_id_ord_map,
    churn_std_hash_map,
    churn_std_btree_map,
    churn_id_hash_map,
    churn_id_ord_map,
    iter_std_hash_map,
    iter_std_btree_map,
    iter_id_hash_map,
    iter_id_ord_map,
    shrink_to_fit_std_hash_map,
    shrink_to_fit_id_hash_map,
    shrink_to_fit_id_ord_map,
    ref_mut_id_ord_map,
);
criterion_main!(benches);
