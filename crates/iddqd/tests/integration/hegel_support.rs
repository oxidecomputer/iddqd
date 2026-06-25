use hegel::{TestCase, generators as gs};
use iddqd_test_utils::{naive_map::NaiveMap, test_item::TestItem};

/// The maximum code point for key2 characters.
///
/// We set a fairly low value here to ensure that more collisions happen in the
/// key2 space.
const MAX_KEY2_CODEPOINT: u8 = 0x7f;

/// Returns a [`TestItem`] with random keys and values.
#[hegel::composite]
pub(crate) fn test_item(tc: TestCase) -> TestItem {
    let key1 = draw_random_key1(&tc);
    let key2 = draw_random_key2(&tc);
    let key3 = draw_random_key3(&tc);
    let value = tc.draw(gs::text());
    TestItem::new(key1, key2, key3, value)
}

/// Draws a batch of non-repeating [`TestItem`]s to fill the map.
pub(crate) fn draw_fill_batch(tc: &TestCase) -> Vec<TestItem> {
    // This is written in this style to try and minimize collisions with as few
    // random draws as possible.
    //
    // Generate a random base for the first key.
    let start = tc.draw(gs::integers::<u8>());
    // Generate a random count of items to fill the map. Bounding the count by
    // MAX_KEY2_CODEPOINT keeps key2 inside the 0..=MAX_KEY2_CODEPOINT codepoint
    // space that random key2 draws also use.
    let count_minus_one =
        tc.draw(gs::integers::<u8>().max_value(MAX_KEY2_CODEPOINT));
    (0..=count_minus_one)
        .map(|i| {
            // key1 stays collision-free because `wrapping_add` of the constant
            // `start` is injective over this i range (there are at most 128
            // values, while u8 can have 256 different values).
            let key1 = start.wrapping_add(i);
            //  Likewise, key2 is collision-free.
            let key2 = char::from(i);
            TestItem::new(key1, key2, format!("fill-{i}"), format!("fill-{i}"))
        })
        .collect()
}

/// Draws a batch of fully random [`TestItem`]s.
///
/// Unlike [`draw_fill_batch`], the keys are drawn at random rather than laid
/// out to avoid collisions. The key1 and key2 spaces are small, so a batch of
/// any appreciable size is likely to contain duplicate keys. This exercises
/// duplicate-key handling, such as serde's rejection of a sequence with
/// repeated keys.
#[cfg(feature = "serde")]
pub(crate) fn draw_random_batch(tc: &TestCase) -> Vec<TestItem> {
    tc.draw(gs::vecs(test_item()).max_size(32))
}

/// Returns a shuffled copy of the given slice.
pub(crate) fn draw_shuffle<T: Clone>(tc: &TestCase, items: &[T]) -> Vec<T> {
    let mut out = items.to_vec();
    if out.len() < 2 {
        return out;
    }
    // This is a simple Fisher-Yates shuffle (Durstenfeld variant, low to high).
    for i in 0..out.len() - 1 {
        let j = tc.draw(
            gs::integers::<usize>().min_value(i).max_value(out.len() - 1),
        );
        out.swap(i, j);
    }
    out
}

/// Draws a random key1 value.
///
/// This is likely to collide with existing key1 values in the map, since the u8
/// space is small.
pub(crate) fn draw_random_key1(tc: &TestCase) -> u8 {
    tc.draw(gs::integers::<u8>())
}

/// Draws a random key2 value.
///
/// This is likely to collide with existing key2 values in the map, since
/// `MAX_KEY2_CODEPOINT` is small.
pub(crate) fn draw_random_key2(tc: &TestCase) -> char {
    tc.draw(gs::characters().max_codepoint(u32::from(MAX_KEY2_CODEPOINT)))
}

/// Draws a random key3 string.
///
/// This is unlikely to match any of the existing key3 strings in the map.
pub(crate) fn draw_random_key3(tc: &TestCase) -> String {
    tc.draw(gs::text())
}

fn draw_lookup_key1_in(tc: &TestCase, items: &[TestItem]) -> u8 {
    if !items.is_empty() && tc.draw(gs::booleans()) {
        let idx = tc.draw(gs::integers::<usize>().max_value(items.len() - 1));
        items[idx].key1
    } else {
        draw_random_key1(tc)
    }
}

fn draw_lookup_key2_in(tc: &TestCase, items: &[TestItem]) -> char {
    if !items.is_empty() && tc.draw(gs::booleans()) {
        let idx = tc.draw(gs::integers::<usize>().max_value(items.len() - 1));
        items[idx].key2
    } else {
        draw_random_key2(tc)
    }
}

fn draw_lookup_key3_in(tc: &TestCase, items: &[TestItem]) -> String {
    if !items.is_empty() && tc.draw(gs::booleans()) {
        let idx = tc.draw(gs::integers::<usize>().max_value(items.len() - 1));
        items[idx].key3.clone()
    } else {
        draw_random_key3(tc)
    }
}

pub(crate) fn draw_lookup_key1(tc: &TestCase, naive: &NaiveMap) -> u8 {
    draw_lookup_key1_in(tc, naive.items())
}

pub(crate) fn draw_lookup_key2(tc: &TestCase, naive: &NaiveMap) -> char {
    draw_lookup_key2_in(tc, naive.items())
}

pub(crate) fn draw_lookup_key3(tc: &TestCase, naive: &NaiveMap) -> String {
    draw_lookup_key3_in(tc, naive.items())
}

pub(crate) fn draw_lookup_keys12(
    tc: &TestCase,
    naive: &NaiveMap,
) -> (u8, char) {
    (
        draw_lookup_key1_in(tc, naive.items()),
        draw_lookup_key2_in(tc, naive.items()),
    )
}

pub(crate) fn draw_lookup_keys123(
    tc: &TestCase,
    naive: &NaiveMap,
) -> (u8, char, String) {
    (
        draw_lookup_key1_in(tc, naive.items()),
        draw_lookup_key2_in(tc, naive.items()),
        draw_lookup_key3_in(tc, naive.items()),
    )
}

#[cfg(any(
    feature = "std",
    all(feature = "default-hasher", feature = "allocator-api2")
))]
pub(crate) const MAX_PANIC_KEY: u32 = 63;

#[cfg(any(
    feature = "std",
    all(feature = "default-hasher", feature = "allocator-api2")
))]
pub(crate) fn draw_armed(tc: &TestCase) -> Option<u32> {
    use iddqd_test_utils::panic_safety::observe_output_path;

    if observe_output_path().is_some() {
        // In observation mode, set the countdown to `u32::MAX` to avoid
        // panicking.
        return Some(u32::MAX);
    }

    // hegel has no native way to do weighted choices, so reconstruct a biased
    // proptest distribution with a bucket selector:
    //
    // * Mostly disarmed so the map gets filled.
    // * When armed, dense coverage of early panics and sparser coverage of
    //   the long tail. The longest observed per-op user-call count is just
    //   over 500 (bulk ops like `retain`/`extend` on a filled map). The 639
    //   ceiling leaves some headroom above that.
    match tc.draw(gs::integers::<u32>().max_value(10)) {
        0..=6 => None,
        7 | 8 => Some(tc.draw(gs::integers::<u32>().max_value(15))),
        9 => Some(tc.draw(gs::integers::<u32>().min_value(16).max_value(127))),
        10.. => {
            Some(tc.draw(gs::integers::<u32>().min_value(128).max_value(639)))
        }
    }
}
