#![feature(async_await, await_macro, futures_api, pin)]

use futures::channel::mpsc;
use futures::future::{ready, FutureExt};
use futures::lock::Mutex;
use futures::stream::StreamExt;
use futures::task::SpawnExt;
use futures_test::future::FutureTestExt;
use futures_test::task::{panic_local_waker_ref, WakeCounter};
use std::sync::Arc;

#[test]
fn mutex_acquire_uncontested() {
    let mutex = Mutex::new(());
    for _ in 0..10 {
        assert!(mutex.lock().poll_unpin(panic_local_waker_ref()).is_ready());
    }
}

#[test]
fn mutex_wakes_waiters() {
    let mutex = Mutex::new(());
    let counter = WakeCounter::new();
    let lock = mutex.lock().poll_unpin(panic_local_waker_ref());
    assert!(lock.is_ready());

    let mut waiter = mutex.lock();
    assert!(waiter.poll_unpin(counter.local_waker()).is_pending());
    assert_eq!(counter.count(), 0);

    drop(lock);

    assert_eq!(counter.count(), 1);
    assert!(waiter.poll_unpin(panic_local_waker_ref()).is_ready());
}

#[test]
fn mutex_contested() {
    let (tx, mut rx) = mpsc::unbounded();
    let mut pool = futures::executor::ThreadPool::builder()
        .pool_size(16)
        .create()
        .unwrap();

    let tx = Arc::new(tx);
    let mutex = Arc::new(Mutex::new(0));

    let num_tasks = 1000;
    for _ in 0..num_tasks {
        let tx = tx.clone();
        let mutex = mutex.clone();
        pool.spawn(async move {
            let mut lock = await!(mutex.lock());
            await!(ready(()).pending_once());
            *lock += 1;
            tx.unbounded_send(()).unwrap();
            drop(lock);
        }).unwrap();
    }

    pool.run(async {
        for _ in 0..num_tasks {
            let () = await!(rx.next()).unwrap();
        }
        let lock = await!(mutex.lock());
        assert_eq!(num_tasks, *lock);
    })
}