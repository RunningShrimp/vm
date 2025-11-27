use std::sync::Arc;
use parking_lot::Mutex;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::collections::HashMap;
use crate::{Jit, CodePtr};
use vm_ir::IRBlock;

pub struct JitPool {
    workers: Vec<thread::JoinHandle<()>>,
    tx: Sender<IRBlock>,
    cache: Arc<Mutex<HashMap<u64, CodePtr>>>,
}

impl JitPool {
    pub fn new(worker_count: usize) -> Self {
        let (tx, rx) = mpsc::channel::<IRBlock>();
        let shared_rx = Arc::new(Mutex::new(rx));
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let mut workers = Vec::new();
        for _ in 0..worker_count {
            let rx = shared_rx.clone();
            let shared = cache.clone();
            let handle = thread::spawn(move || {
                let mut jit = Jit::new().with_pool_cache(shared.clone());
                loop {
                    let res = {
                        let guard = rx.lock();
                        guard.recv()
                    };
                    match res {
                        Ok(block) => { let _ = jit.compile(&block); }
                        Err(_) => break,
                    }
                }
            });
            workers.push(handle);
        }
        Self { workers, tx, cache }
    }

    pub fn submit(&self, blocks: Vec<IRBlock>) {
        for b in blocks { let _ = self.tx.send(b); }
    }

    pub fn cache(&self) -> Arc<Mutex<HashMap<u64, CodePtr>>> { self.cache.clone() }
}

impl Drop for JitPool {
    fn drop(&mut self) {
        // Workers will exit when channel is closed
        for h in self.workers.drain(..) {
            let _ = h.join();
        }
    }
}
