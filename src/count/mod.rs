use std::thread;
use std::time::Duration;

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

pub mod count_lim;

static mut COUNTS: [usize; 128] = [0usize; 128];
static mut COUNTS_EVENTUAL: [usize; 128] = [0usize; 128];
static mut GLOBAL_COUNT_EVENTUAL: usize = 0;
static mut COUNTS_END: [Option<*mut usize>; 32] = [None; 32];

pub unsafe fn count_stat() -> usize {
    let threads: Vec<_> = (0..32)
        .map(|i| {
            thread::spawn(move || {
                for _ in 0..100 {
                    thread::sleep(Duration::from_micros(fastrand::u64(5..15)));
                    COUNTS[i] += 1;
                }
            })
        })
        .collect();
    for t in threads {
        t.join().ok();
    }
    COUNTS[..32].iter().sum()
}

pub unsafe fn count_stat_eventual() -> usize {
    let threads: Vec<_> = (0..32)
        .map(|i| {
            thread::spawn(move || {
                for _ in 0..100 {
                    thread::sleep(Duration::from_micros(fastrand::u64(5..15)));
                    COUNTS_EVENTUAL[i] += 1;
                }
            })
        })
        .collect();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag2 = stop_flag.clone();
    let eventual = thread::spawn(move || {
        while !stop_flag2.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(1));
            GLOBAL_COUNT_EVENTUAL = COUNTS[..32].iter().sum();
        }
    });
    for t in threads {
        t.join().ok();
    }
    stop_flag.store(true, Ordering::SeqCst);
    eventual.join().ok();
    GLOBAL_COUNT_EVENTUAL
}

pub unsafe fn count_end() -> usize {
    let final_count = Arc::new(AtomicUsize::new(0));
    thread_local!(static COUNT:RefCell<usize> = RefCell::new(0));
    let threads: Vec<_> = (0..32)
        .map(|i| {
            let final_count = final_count.clone();
            thread::spawn(move || {
                COUNT.with(|f| {
                    COUNTS_END[i] = Some(f.as_ptr());
                });
                for _ in 0..100 {
                    thread::sleep(Duration::from_micros(fastrand::u64(5..15)));
                    COUNT.with(|count| {
                        *count.borrow_mut() += 1;
                    })
                }
                final_count.fetch_add(100, Ordering::SeqCst);
                COUNTS_END[i] = None;
            })
        })
        .collect();
    for t in threads {
        t.join().ok();
    }
    let sum = COUNTS_END.iter().fold(0usize, |prev, count| {
        if let Some(count) = count {
            prev + **count
        } else {
            prev
        }
    });
    sum + final_count.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn count_atomic() {
        let counter = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..10)
            .map(|_| {
                let counter = counter.clone();
                thread::spawn(move || {
                    for _ in 0..100 {
                        thread::sleep(Duration::from_micros(fastrand::u64(5..15)));
                        counter.fetch_add(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().ok();
        }
        assert_eq!(counter.load(Ordering::SeqCst), 1000);
    }

    #[test]
    fn count_stat() {
        unsafe {
            assert_eq!(super::count_stat(), 3200);
        }
    }

    #[test]
    fn count_stat_eventual() {
        unsafe {
            assert_eq!(super::count_stat_eventual(), 3200);
        }
    }

    #[test]
    fn count_end() {
        unsafe {
            assert_eq!(super::count_end(), 3200);
        }
    }
}
