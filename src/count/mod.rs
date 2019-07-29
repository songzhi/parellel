#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;
    use rand::random;

    #[test]
    fn count_atomic() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::new();
        for _ in 0..10 {
            let counter = counter.clone();
            threads.push(thread::spawn(move || {
                for _ in 0..100 {
                    thread::sleep(Duration::from_micros(random::<u8>() as u64));
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            }))
        }
        for t in threads {
            t.join();
        }
        assert_eq!(counter.load(Ordering::SeqCst), 1000);
    }
}