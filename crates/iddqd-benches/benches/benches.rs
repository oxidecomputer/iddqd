//! Benchmarks for iddqd.
//!
//! This is very elementary at the moment. In the future, more benchmarks will
//! live here.

use criterion::{Criterion, criterion_group, criterion_main};
use iddqd::{DefaultHashBuilder, IdHashMap, IdOrdMap};
use iddqd_benches::{RecordBorrowedU32, RecordOwnedU32};
use iddqd_test_utils::test_item::{TestItem, TestKey1};
use std::collections::{BTreeMap, HashMap};

fn bench_fn(c: &mut Criterion) {
    // Benchmark the id_ord_map::RefMut implementation with a very simple hash
    // function.
    //
    // This aims to benchmark the overhead of RefMut itself, without considering
    // how long the hash function takes.
    c.bench_function("id_ord_map_ref_mut_simple", |b| {
        b.iter_batched(
            || {
                // Create a new IdOrdMap instance.
                let mut map = IdOrdMap::new();
                map.insert_overwrite(TestItem::new(1, 'a', "foo", "bar"));
                map
            },
            |mut map| {
                let mut item = map.get_mut(&TestKey1::new(&1)).unwrap();
                item.key2 = 'b';
                drop(item);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("hash_map_u32_get", |b| {
        b.iter_batched(
            || {
                let mut map =
                    HashMap::with_hasher(DefaultHashBuilder::default());
                map.insert(
                    0u32,
                    RecordOwnedU32 { index: 0, data: "data".to_owned() },
                );
                map.insert(
                    1u32,
                    RecordOwnedU32 { index: 1, data: "data1".to_owned() },
                );
                map
            },
            |map| {
                map.get(&0);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("id_hash_map_owned_u32_get", |b| {
        b.iter_batched(
            || {
                // Create a new IdHashMap instance.
                let mut map = IdHashMap::new();
                map.insert_overwrite(RecordOwnedU32 {
                    index: 0,
                    data: "data".to_owned(),
                });
                map.insert_overwrite(RecordOwnedU32 {
                    index: 1,
                    data: "data1".to_owned(),
                });
                map
            },
            |map| {
                map.get(&0);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("id_hash_map_borrowed_u32_get", |b| {
        b.iter_batched(
            || {
                // Create a new IdHashMap instance.
                let mut map = IdHashMap::new();
                map.insert_overwrite(RecordBorrowedU32 {
                    index: 0,
                    data: "data".to_owned(),
                });
                map.insert_overwrite(RecordBorrowedU32 {
                    index: 1,
                    data: "data1".to_owned(),
                });
                map
            },
            |map| {
                map.get(&0);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("btree_map_u32_get", |b| {
        b.iter_batched(
            || {
                let mut map = BTreeMap::new();
                map.insert(
                    0u32,
                    RecordOwnedU32 { index: 0, data: "data".to_owned() },
                );
                map.insert(
                    1u32,
                    RecordOwnedU32 { index: 1, data: "data1".to_owned() },
                );
                map
            },
            |map| {
                map.get(&0);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("id_ord_map_owned_u32_get", |b| {
        b.iter_batched(
            || {
                // Create a new IdHashMap instance.
                let mut map = IdOrdMap::new();
                map.insert_overwrite(RecordOwnedU32 {
                    index: 0,
                    data: "data".to_owned(),
                });
                map.insert_overwrite(RecordOwnedU32 {
                    index: 1,
                    data: "data1".to_owned(),
                });
                map
            },
            |map| {
                map.get(&0);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    c.bench_function("id_ord_map_borrowed_u32_get", |b| {
        b.iter_batched(
            || {
                // Create a new IdHashMap instance.
                let mut map = IdOrdMap::new();
                map.insert_overwrite(RecordBorrowedU32 {
                    index: 0,
                    data: "data".to_owned(),
                });
                map.insert_overwrite(RecordBorrowedU32 {
                    index: 1,
                    data: "data1".to_owned(),
                });
                map
            },
            |map| {
                map.get(&0);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_fn);
criterion_main!(benches);
