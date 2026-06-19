use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::task::{Poll, Waker};
use std::sync::{Mutex, Once};

use crate::{WaitQueue, api as axtask, current};

static INIT: Once = Once::new();
static SERIAL: Mutex<()> = Mutex::new(());

#[test]
fn test_sched_fifo() {
    let _lock = SERIAL.lock();
    INIT.call_once(axtask::init_scheduler);

    const NUM_TASKS: usize = 10;
    static FINISHED_TASKS: AtomicUsize = AtomicUsize::new(0);

    for i in 0..NUM_TASKS {
        axtask::spawn_raw(
            move || {
                println!("sched-fifo: Hello, task {}! ({})", i, current().id_name());
                axtask::yield_now();
                let order = FINISHED_TASKS.fetch_add(1, Ordering::Release);
                assert_eq!(order, i); // FIFO scheduler
            },
            format!("T{}", i),
            0x1000,
        );
    }

    while FINISHED_TASKS.load(Ordering::Acquire) < NUM_TASKS {
        axtask::yield_now();
    }
}

#[test]
fn test_fp_state_switch() {
    let _lock = SERIAL.lock();
    INIT.call_once(axtask::init_scheduler);

    const NUM_TASKS: usize = 5;
    const FLOATS: [f64; NUM_TASKS] = [
        3.141592653589793,
        2.718281828459045,
        -1.4142135623730951,
        0.0,
        0.618033988749895,
    ];
    static FINISHED_TASKS: AtomicUsize = AtomicUsize::new(0);

    for (i, float) in FLOATS.iter().enumerate() {
        axtask::spawn(move || {
            let mut value = float + i as f64;
            axtask::yield_now();
            value -= i as f64;

            println!("fp_state_switch: Float {} = {}", i, value);
            assert!((value - float).abs() < 1e-9);
            FINISHED_TASKS.fetch_add(1, Ordering::Release);
        });
    }
    while FINISHED_TASKS.load(Ordering::Acquire) < NUM_TASKS {
        axtask::yield_now();
    }
}

#[test]
fn test_wait_queue() {
    let _lock = SERIAL.lock();
    INIT.call_once(axtask::init_scheduler);

    const NUM_TASKS: usize = 10;

    static WQ1: WaitQueue = WaitQueue::new();
    static WQ2: WaitQueue = WaitQueue::new();
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    for _ in 0..NUM_TASKS {
        axtask::spawn(move || {
            COUNTER.fetch_add(1, Ordering::Release);
            println!("wait_queue: task {:?} started", current().id());
            WQ1.notify_one(true); // WQ1.wait_until()
            WQ2.wait();

            assert!(!current().in_wait_queue());

            COUNTER.fetch_sub(1, Ordering::Release);
            println!("wait_queue: task {:?} finished", current().id());
            WQ1.notify_one(true); // WQ1.wait_until()
        });
    }

    println!("task {:?} is waiting for tasks to start...", current().id());
    WQ1.wait_until(|| COUNTER.load(Ordering::Acquire) == NUM_TASKS);
    assert_eq!(COUNTER.load(Ordering::Acquire), NUM_TASKS);
    assert!(!current().in_wait_queue());
    WQ2.notify_all(true); // WQ2.wait()

    println!(
        "task {:?} is waiting for tasks to finish...",
        current().id()
    );
    WQ1.wait_until(|| COUNTER.load(Ordering::Acquire) == 0);
    assert_eq!(COUNTER.load(Ordering::Acquire), 0);
    assert!(!current().in_wait_queue());
}

#[test]
fn test_task_join() {
    let _lock = SERIAL.lock();
    INIT.call_once(axtask::init_scheduler);

    const NUM_TASKS: usize = 10;
    let mut tasks = Vec::with_capacity(NUM_TASKS);

    for i in 0..NUM_TASKS {
        tasks.push(axtask::spawn_raw(
            move || {
                println!("task_join: task {}! ({})", i, current().id_name());
                axtask::yield_now();
                axtask::exit(i as _);
            },
            format!("T{}", i),
            0x1000,
        ));
    }

    for i in 0..NUM_TASKS {
        assert_eq!(tasks[i].join(), Some(i as _));
    }
}

#[test]
fn test_future_block_on_immediate_ready() {
    let _lock = SERIAL.lock();
    INIT.call_once(axtask::init_scheduler);

    let result = crate::block_on(async { 42u32 });
    assert_eq!(result, 42);
}

#[test]
fn test_future_block_on_multi_pending() {
    let _lock = SERIAL.lock();
    INIT.call_once(axtask::init_scheduler);

    static POLL_COUNT: AtomicUsize = AtomicUsize::new(0);
    const TARGET: usize = 5;

    POLL_COUNT.store(0, Ordering::Relaxed);

    let result = crate::block_on(core::future::poll_fn(|cx| {
        let count = POLL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= TARGET {
            Poll::Ready(count)
        } else {
            let w = cx.waker().clone();
            axtask::spawn(move || {
                axtask::yield_now();
                w.wake();
            });
            Poll::Pending
        }
    }));

    assert_eq!(result, TARGET);
    assert!(POLL_COUNT.load(Ordering::Relaxed) >= TARGET);
}

#[test]
fn test_future_block_on_wake_from_peer() {
    let _lock = SERIAL.lock();
    INIT.call_once(axtask::init_scheduler);

    static FLAG: AtomicBool = AtomicBool::new(false);
    static WAKER: Mutex<Option<Waker>> = Mutex::new(None);

    FLAG.store(false, Ordering::Relaxed);

    let result = crate::block_on(core::future::poll_fn(|cx| {
        if FLAG.load(Ordering::Relaxed) {
            Poll::Ready(true)
        } else {
            *WAKER.lock().unwrap() = Some(cx.waker().clone());
            axtask::spawn(|| {
                FLAG.store(true, Ordering::Relaxed);
                let w = WAKER.lock().unwrap().take().unwrap();
                w.wake();
            });
            Poll::Pending
        }
    }));

    assert!(result);
}

#[test]
fn test_future_block_on_self_wake() {
    let _lock = SERIAL.lock();
    INIT.call_once(axtask::init_scheduler);

    static POLL_COUNT: AtomicUsize = AtomicUsize::new(0);
    static WAKER: Mutex<Option<Waker>> = Mutex::new(None);

    POLL_COUNT.store(0, Ordering::Relaxed);

    let result = crate::block_on(core::future::poll_fn(|cx| {
        let count = POLL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= 2 {
            Poll::Ready(count)
        } else {
            *WAKER.lock().unwrap() = Some(cx.waker().clone());
            let w = WAKER.lock().unwrap().clone().unwrap();
            w.wake_by_ref();
            Poll::Pending
        }
    }));

    assert!(result >= 2);
    assert!(POLL_COUNT.load(Ordering::Relaxed) >= 2);
}

#[test]
fn test_future_block_on_duplicate_wake() {
    let _lock = SERIAL.lock();
    INIT.call_once(axtask::init_scheduler);

    static WAKER: Mutex<Option<Waker>> = Mutex::new(None);

    let result = crate::block_on(core::future::poll_fn(|cx| {
        *WAKER.lock().unwrap() = Some(cx.waker().clone());
        let w = WAKER.lock().unwrap().clone().unwrap();
        w.wake_by_ref();
        w.wake_by_ref();
        w.wake_by_ref();
        Poll::Ready(99u32)
    }));

    assert_eq!(result, 99);
}

#[test]
fn test_future_block_on_expired_waker() {
    let _lock = SERIAL.lock();
    INIT.call_once(axtask::init_scheduler);

    static WAKER: Mutex<Option<Waker>> = Mutex::new(None);

    crate::block_on(async {
        let w = crate::current().clone();
        let waker_data = alloc::sync::Arc::new(crate::future::AxWaker::new(alloc::sync::Arc::downgrade(&w)));
        *WAKER.lock().unwrap() = Some(waker_data.into_raw_waker());
    });

    let waker = WAKER.lock().unwrap().take().unwrap();
    waker.wake_by_ref();
    waker.clone().wake();
    drop(waker);
}
