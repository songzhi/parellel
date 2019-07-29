use std::sync::atomic::{Ordering, AtomicUsize};
use std::sync::{Arc, Mutex};
use std::cell::RefCell;
use std::thread;
use std::ops::DerefMut;

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
    pub fn add_count(&mut self, delta: usize) -> bool {
        let added = COUNTER_WITH_MAX.with(|data| -> bool {
            let mut data = data.borrow_mut();
            let (counter, counter_max) = data.deref_mut();
            if *counter_max - *counter >= delta {
                *counter += delta;
                true
            } else { false }
        });
        if added { return true; }
        self.globalize_count();
        if self.count_max - self.count - self.reserve > delta {
            return false;
        }
        self.count += delta;
        self.balance_count();
        true
    }
    pub fn sub_count(&mut self, delta: usize) -> bool {
        let subed = COUNTER_WITH_MAX.with(|data| -> bool {
            let mut data = data.borrow_mut();
            let (counter, _) = data.deref_mut();
            if *counter >= delta {
                *counter -= delta;
                true
            } else { false }
        });
        if subed { return true; }
        self.globalize_count();
        if self.count < delta {
            return false;
        }
        self.count -= delta;
        self.balance_count();
        true
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

pub type GlobalDataInstance = Arc<RefCell<Mutex<GlobalData>>>;
