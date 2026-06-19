extern crate alloc;

use alloc::sync::{Arc, Weak};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use crate::{current_run_queue, select_run_queue, yield_now};

type WeakTaskRef = Weak<crate::AxTask>;

pub(crate) struct AxWaker {
    task: WeakTaskRef,
    woke: kspin::SpinNoIrq<bool>,
}

impl AxWaker {
    pub(crate) fn new(task: WeakTaskRef) -> Self {
        Self {
            task,
            woke: kspin::SpinNoIrq::new(false),
        }
    }

    fn wake_inner(&self) {
        *self.woke.lock() = true;
        if let Some(task) = self.task.upgrade() {
            select_run_queue::<kernel_guard::NoPreemptIrqSave>(&task).unblock_task(task, false);
        }
    }

    pub(crate) fn into_raw_waker(self: &Arc<Self>) -> Waker {
        let data = Arc::into_raw(self.clone()) as *const ();
        unsafe {
            Waker::from_raw(RawWaker::new(data, &VTABLE))
        }
    }
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    |data| {
        let arc = unsafe { Arc::from_raw(data as *const AxWaker) };
        let cloned = arc.clone();
        let _ = Arc::into_raw(arc);
        RawWaker::new(Arc::into_raw(cloned) as *const (), &VTABLE)
    },
    |data| {
        let arc = unsafe { Arc::from_raw(data as *const AxWaker) };
        arc.wake_inner();
    },
    |data| {
        let arc = unsafe { Arc::from_raw(data as *const AxWaker) };
        arc.wake_inner();
        let _ = Arc::into_raw(arc);
    },
    |data| {
        drop(unsafe { Arc::from_raw(data as *const AxWaker) });
    },
);

pub fn block_on<F: Future>(future: F) -> F::Output {
    let task_ref = crate::current().clone();
    let waker_data = Arc::new(AxWaker::new(Arc::downgrade(&task_ref)));
    let raw_waker = waker_data.into_raw_waker();
    let mut cx = Context::from_waker(&raw_waker);

    let mut future = future;
    let mut pinned = unsafe { Pin::new_unchecked(&mut future) };

    loop {
        *waker_data.woke.lock() = false;

        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => {}
        }

        let woke_guard = waker_data.woke.lock();
        if *woke_guard {
            drop(woke_guard);
            yield_now();
        } else {
            let mut rq = current_run_queue::<kernel_guard::NoPreemptIrqSave>();
            if *woke_guard {
                drop(woke_guard);
                drop(rq);
                yield_now();
            } else {
                // woke_guard held during set_state to prevent lost wake,
                // then dropped BEFORE resched to prevent waker deadlock.
                rq.block_current(woke_guard);
            }
        }
    }
}
