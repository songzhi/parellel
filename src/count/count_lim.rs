use std::cell::RefCell;
use std::ops::DerefMut;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// 每线程变量counter和counter_max各自对应线程的本地计数和计数上限.
thread_local!(static COUNTER_WITH_MAX: RefCell<(usize,usize)> = RefCell::new((0,0)));
static mut COUNTER_PTR: [Option<*mut usize>; 32] = [None; 32];
static ACTIVE_THREADS_COUNT: AtomicUsize = AtomicUsize::new(0);

pub struct GlobalData {
    /// 合计计数上限
    count_max: usize,
    /// 全局计数
    count: usize,
    /// 所有每线程变量counter_max的和
    reserve: usize,
}

impl Default for GlobalData {
    fn default() -> Self {
        Self {
            count_max: 10000,
            count: 0,
            reserve: 0,
        }
    }
}

impl GlobalData {
    fn globalize_count(&mut self) {
        COUNTER_WITH_MAX.with(|data| {
            let mut data = data.borrow_mut();
            let (counter, counter_max) = data.deref_mut();
            self.count += *counter;
            *counter = 0;
            self.reserve -= *counter_max;
            *counter_max = 0;
        });
    }
    fn balance_count(&mut self) {
        COUNTER_WITH_MAX.with(|data| {
            let mut data = data.borrow_mut();
            let (counter, counter_max) = data.deref_mut();
            *counter_max = self.count_max - self.count - self.reserve;
            *counter_max /= ACTIVE_THREADS_COUNT.load(Ordering::SeqCst);
            self.reserve += *counter_max;
            *counter = *counter_max / 2;
            if *counter > self.count {
                *counter = self.count;
            }
            self.count -= *counter;
        });
    }
    pub unsafe fn read_count(&self) -> usize {
        let mut sum = self.count;
        sum += COUNTER_PTR.iter().fold(0usize, |prev, count| {
            if let Some(count) = count {
                prev + **count
            } else {
                prev
            }
        });
        sum
    }
}

pub type GlobalDataInstance = Arc<Mutex<GlobalData>>;

pub fn add_count(global_data: &GlobalDataInstance, delta: usize) -> bool {
    let added = COUNTER_WITH_MAX.with(|data| -> bool {
        let mut data = data.borrow_mut();
        let (counter, counter_max) = data.deref_mut();
        if *counter_max - *counter >= delta {
            *counter += delta;
            true
        } else {
            false
        }
    });
    if added {
        return true;
    }
    let mut global_data = global_data.lock().unwrap();
    global_data.globalize_count();
    if global_data.count_max - global_data.count - global_data.reserve < delta {
        return false;
    }
    global_data.count += delta;
    global_data.balance_count();
    true
}

pub fn sub_count(global_data: &GlobalDataInstance, delta: usize) -> bool {
    let subed = COUNTER_WITH_MAX.with(|data| -> bool {
        let mut data = data.borrow_mut();
        let (counter, _) = data.deref_mut();
        if *counter >= delta {
            *counter -= delta;
            true
        } else {
            false
        }
    });
    if subed {
        return true;
    }
    let mut global_data = global_data.lock().unwrap();
    global_data.globalize_count();
    if global_data.count < delta {
        return false;
    }
    global_data.count -= delta;
    global_data.balance_count();
    true
}

pub unsafe fn count_register_thread(global_data: &GlobalDataInstance, index: usize) {
    let _ = global_data.lock().unwrap();
    COUNTER_WITH_MAX.with(|data| {
        let mut data = data.borrow_mut();
        let (counter, _) = data.deref_mut();
        COUNTER_PTR[index] = Some(counter as *mut usize);
    });
    ACTIVE_THREADS_COUNT.fetch_add(1, Ordering::SeqCst);
}

pub unsafe fn count_unregister_thread(global_data: &GlobalDataInstance, index: usize) {
    let mut global_data = global_data.lock().unwrap();
    global_data.globalize_count();
    COUNTER_PTR[index] = None;
    ACTIVE_THREADS_COUNT.fetch_sub(1, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn main() {
        let global_data = Arc::new(Mutex::new(GlobalData::default()));
        let threads: Vec<_> = (0..32)
            .map(|i| {
                let global_data = global_data.clone();
                thread::spawn(move || {
                    unsafe {
                        count_register_thread(&global_data, i);
                    }
                    for _ in 0..100 {
                        thread::sleep(Duration::from_micros(fastrand::u64(5..15)));
                        add_count(&global_data, 1);
                    }
                    unsafe {
                        count_unregister_thread(&global_data, i);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().ok();
        }
        let global_data = global_data.lock().unwrap();
        assert_eq!(unsafe { global_data.read_count() }, 3200);
    }
}
