use std::panic::AssertUnwindSafe;

pub fn catch_panic<T>(f: impl FnOnce() -> T) -> Option<T> {
    let result = std::panic::catch_unwind(AssertUnwindSafe(f));
    match result {
        Ok(value) => Some(value),
        Err(err) => {
            if let Some(err) = err.downcast_ref::<&str>() {
                eprintln!("caught panic: {}", err);
            } else {
                eprintln!("caught unknown panic");
            }
            None
        }
    }
}
