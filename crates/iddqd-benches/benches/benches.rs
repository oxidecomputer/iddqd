//! Benchmarks for iddqd.
//!
//! This is very elementary at the moment. In the future, more benchmarks will
//! live here.

use criterion::{Criterion, criterion_group, criterion_main};
use iddqd::IdOrdMap;
use iddqd_test_utils::test_item::{TestItem, TestKey1};

fn bench_fn(c: &mut Criterion) {
    // Benchmark the id_ord_map::RefMut implementation with a very simple hash
    // function.
    //
    // This aims to benchmark the overhead of RefMut itself, without considering
    // how long the hash function takes.
    c.bench_function("id_ord_map_ref_mut_simple", |b| {
        b.iter_batched(
            || {
                //
                // Create a new IdOrdMap instance
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
}

criterion_group!(benches, bench_fn);
criterion_main!(benches);
