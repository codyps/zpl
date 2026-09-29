//! Bounded parallelism for independent, offline capture replays.
use std::{panic::resume_unwind, thread};

/// Map in input order, with a fixed upper bound on concurrently resident rasters.
/// Jobs share no renderer state. Worker failures propagate to the test harness.
/// `ZPL_RASTER_THREADS=1` selects serial replay for profiling and reproducibility.
pub fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let available = thread::available_parallelism().map_or(1, usize::from);
    let workers = match std::env::var("ZPL_RASTER_THREADS") {
        Ok(value) => value
            .parse::<usize>()
            .expect("ZPL_RASTER_THREADS must be an integer"),
        Err(std::env::VarError::NotPresent) => available.min(8),
        Err(error) => panic!("invalid ZPL_RASTER_THREADS: {error}"),
    };
    assert!(
        (1..=8).contains(&workers),
        "ZPL_RASTER_THREADS must be between 1 and 8"
    );
    map_with_workers(items, workers, f)
}

fn map_with_workers<T: Sync, R: Send>(
    items: &[T],
    workers: usize,
    f: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    if items.is_empty() {
        return Vec::new();
    }
    if workers == 1 {
        return items.iter().map(f).collect();
    }
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    thread::scope(|scope| {
        let handles: Vec<_> = (0..workers.min(items.len()))
            .map(|_| {
                let (next, f) = (&next, &f);
                scope.spawn(move || {
                    let mut results = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        results.push((i, f(item)));
                    }
                    results
                })
            })
            .collect();
        let mut results: Vec<_> = handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_else(|e| resume_unwind(e)))
            .collect();
        results.sort_unstable_by_key(|(i, _)| *i);
        results.into_iter().map(|(_, result)| result).collect()
    })
}

#[test]
fn worker_counts_preserve_order_and_each_input_once() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let visits: Vec<_> = (0..37).map(|_| AtomicUsize::new(0)).collect();
    let input: Vec<_> = (0..37).collect();
    for workers in [1, 2, 8] {
        let actual = map_with_workers(&input, workers, |&i| {
            visits[i].fetch_add(1, Ordering::Relaxed);
            i * i
        });
        assert_eq!(actual, input.iter().map(|i| i * i).collect::<Vec<_>>());
    }
    assert!(visits.iter().all(|v| v.load(Ordering::Relaxed) == 3));
    assert!(map_with_workers::<usize, usize>(&[], 8, |&i| i).is_empty());
}

#[test]
fn worker_panic_fails_the_caller() {
    let result = std::panic::catch_unwind(|| {
        map_with_workers(&[0, 1, 2], 2, |&i| {
            assert_ne!(i, 1, "fixture failure");
            i
        })
    });
    assert!(result.is_err());
}
