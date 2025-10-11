//! Benchmarks for iddqd.
//!
//! This is very elementary at the moment. In the future, more benchmarks will
//! live here.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iddqd::{DefaultHashBuilder, IdHashMap, IdOrdMap};
use iddqd_benches::{RecordBorrowedU32, RecordOwnedU32};
use iddqd_test_utils::test_item::{TestItem, TestKey1};
use std::collections::{BTreeMap, HashMap};

const SIZES: &[usize] =
    &[1, 10, 100, 1_000, 10_000, 50_000, 100_000, 500_000, 1_000_000];

fn bench_fn(c: &mut Criterion) {
    // Benchmark the id_ord_map::RefMut implementation with a very simple hash
    // function.
    //
    // This aims to benchmark the overhead of RefMut itself, without considering
    // how long the hash function takes.
    c.bench_function("id_ord_map_ref_mut_simple", |b| {
        b.iter_batched_ref(
            || {
                // Create a new IdOrdMap instance.
                let mut map = IdOrdMap::new();
                map.insert_overwrite(TestItem::new(1, 'a', "foo", "bar"));
                map
            },
            |map| {
                let mut item = map.get_mut(&TestKey1::new(&1)).unwrap();
                item.key2 = 'b';
                drop(item);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    let mut group = c.benchmark_group("hash_map_u32_get");
    for size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter_batched_ref(
                    || {
                        let mut map =
                            HashMap::with_hasher(DefaultHashBuilder::default());
                        for i in 0..size as u32 {
                            map.insert(
                                i,
                                RecordOwnedU32 {
                                    index: i,
                                    data: format!("data{}", i),
                                },
                            );
                        }
                        map
                    },
                    |map| {
                        map.get(&0);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();

    let mut group = c.benchmark_group("id_hash_map_owned_u32_get");
    for size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter_batched_ref(
                    || {
                        // Create a new IdHashMap instance.
                        let mut map = IdHashMap::new();
                        for i in 0..size as u32 {
                            map.insert_overwrite(RecordOwnedU32 {
                                index: i,
                                data: format!("data{}", i),
                            });
                        }
                        map
                    },
                    |map| {
                        map.get(&0);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();

    let mut group = c.benchmark_group("id_hash_map_borrowed_u32_get");
    for size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter_batched_ref(
                    || {
                        // Create a new IdHashMap instance.
                        let mut map = IdHashMap::new();
                        for i in 0..size as u32 {
                            map.insert_overwrite(RecordBorrowedU32 {
                                index: i,
                                data: format!("data{}", i),
                            });
                        }
                        map
                    },
                    |map| {
                        map.get(&0);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();

    let mut group = c.benchmark_group("btree_map_u32_get");
    for size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter_batched_ref(
                    || {
                        let mut map = BTreeMap::new();
                        for i in 0..size as u32 {
                            map.insert(
                                i,
                                RecordOwnedU32 {
                                    index: i,
                                    data: format!("data{}", i),
                                },
                            );
                        }
                        map
                    },
                    |map| {
                        map.get(&0);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();

    let mut group = c.benchmark_group("id_ord_map_owned_u32_get");
    for size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter_batched_ref(
                    || {
                        // Create a new IdOrdMap instance.
                        let mut map = IdOrdMap::new();
                        for i in 0..size as u32 {
                            map.insert_overwrite(RecordOwnedU32 {
                                index: i,
                                data: format!("data{}", i),
                            });
                        }
                        map
                    },
                    |map| {
                        map.get(&0);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();

    let mut group = c.benchmark_group("id_ord_map_borrowed_u32_get");
    for size in SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter_batched_ref(
                    || {
                        // Create a new IdOrdMap instance.
                        let mut map = IdOrdMap::new();
                        for i in 0..size as u32 {
                            map.insert_overwrite(RecordBorrowedU32 {
                                index: i,
                                data: format!("data{}", i),
                            });
                        }
                        map
                    },
                    |map| {
                        map.get(&0);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_fn);
criterion_main!(benches);
