use std::sync::{Mutex, MutexGuard};

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    #[test]
    fn poisoned_runtime_state_remains_accessible() {
        let state = Arc::new(Mutex::new(1));
        let worker_state = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let _guard = worker_state.lock().expect("initial lock");
            panic!("poison runtime state");
        })
        .join();

        *super::lock(&state) = 2;
        assert_eq!(*super::lock(&state), 2);
    }
}
