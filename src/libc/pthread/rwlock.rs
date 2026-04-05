use crate::Environment;
use crate::mem::MutVoidPtr;
use std::sync::Mutex;

// Global lock to enforce safe atomic memory access on Android ARM processors
static RWLOCK_SYNC: Mutex<()> = Mutex::new(());

pub fn pthread_rwlock_init(env: &mut Environment, rwlock: MutVoidPtr, _attr: MutVoidPtr) -> u32 {
    if !rwlock.is_null() {
        let _guard = RWLOCK_SYNC.lock().unwrap();
        env.mem.write(rwlock.cast::<u32>(), 0);
    }
    0
}

pub fn pthread_rwlock_destroy(_env: &mut Environment, _rwlock: MutVoidPtr) -> u32 {
    0
}

pub fn pthread_rwlock_tryrdlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    if rwlock.is_null() { return 22; } // EINVAL
    let _guard = RWLOCK_SYNC.lock().unwrap();
    let state_ptr = rwlock.cast::<u32>();
    let state = env.mem.read(state_ptr);
    if state == 0xFFFFFFFF {
        return 16; // EBUSY
    }
    env.mem.write(state_ptr, state + 1);
    0
}

pub fn pthread_rwlock_rdlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    loop {
        let res = pthread_rwlock_tryrdlock(env, rwlock);
        if res != 16 {
            return res; // Success or fatal error
        }
        // Yield the CPU thread for 1 millisecond so the background thread can finish loading
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

pub fn pthread_rwlock_trywrlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    if rwlock.is_null() { return 22; } // EINVAL
    let _guard = RWLOCK_SYNC.lock().unwrap();
    let state_ptr = rwlock.cast::<u32>();
    let state = env.mem.read(state_ptr);
    if state != 0 {
        return 16; // EBUSY
    }
    env.mem.write(state_ptr, 0xFFFFFFFF);
    0
}

pub fn pthread_rwlock_wrlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    loop {
        let res = pthread_rwlock_trywrlock(env, rwlock);
        if res != 16 {
            return res; // Success or fatal error
        }
        // Yield the CPU thread for 1 millisecond so the background thread can finish loading
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

pub fn pthread_rwlock_unlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    if rwlock.is_null() { return 22; } // EINVAL
    let _guard = RWLOCK_SYNC.lock().unwrap();
    let state_ptr = rwlock.cast::<u32>();
    let state = env.mem.read(state_ptr);
    if state == 0 {
        // Already unlocked
    } else if state == 0xFFFFFFFF {
        env.mem.write(state_ptr, 0); // Unlock writer
    } else {
        env.mem.write(state_ptr, state - 1); // Unlock reader
    }
    0
}

// --- CORRECT EXPORT FORMAT FOR YOUR FORK ---
pub const FUNCTIONS: crate::dyld::FunctionExports = &[
    crate::export_c_func!(pthread_rwlock_init(_, _)),
    crate::export_c_func!(pthread_rwlock_destroy(_)),
    crate::export_c_func!(pthread_rwlock_tryrdlock(_)),
    crate::export_c_func!(pthread_rwlock_rdlock(_)),
    crate::export_c_func!(pthread_rwlock_trywrlock(_)),
    crate::export_c_func!(pthread_rwlock_wrlock(_)),
    crate::export_c_func!(pthread_rwlock_unlock(_)),
];
