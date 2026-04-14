use crate::Environment;
use crate::mem::MutVoidPtr;

pub fn pthread_rwlock_init(env: &mut Environment, rwlock: MutVoidPtr, _attr: MutVoidPtr) -> u32 {
    if !rwlock.is_null() {
        env.mem.write(rwlock.cast::<u32>(), 0);
    }
    0
}

pub fn pthread_rwlock_destroy(_env: &mut Environment, _rwlock: MutVoidPtr) -> u32 {
    0
}

pub fn pthread_rwlock_tryrdlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    if rwlock.is_null() { return 22; } // EINVAL
    let state_ptr = rwlock.cast::<u32>();
    let state = env.mem.read(state_ptr);
    if state == 0xFFFFFFFF {
        return 16; // EBUSY (A writer holds the lock)
    }
    // Allow multiple readers! Just increment the reader count.
    env.mem.write(state_ptr, state + 1);
    0
}

pub fn pthread_rwlock_rdlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    let mut spins = 0;
    loop {
        if pthread_rwlock_tryrdlock(env, rwlock) == 0 {
            return 0; // Successfully grabbed the read lock
        }
        spins += 1;
        if spins > 10 {
            // Safely yield the host thread so the emulator doesn't choke
            std::thread::yield_now(); 
        }
    }
}

pub fn pthread_rwlock_trywrlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    if rwlock.is_null() { return 22; } // EINVAL
    let state_ptr = rwlock.cast::<u32>();
    let state = env.mem.read(state_ptr);
    if state != 0 {
        return 16; // EBUSY (Readers or another writer hold the lock)
    }
    // Lock exclusively for writing
    env.mem.write(state_ptr, 0xFFFFFFFF);
    0
}

pub fn pthread_rwlock_wrlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    let mut spins = 0;
    loop {
        if pthread_rwlock_trywrlock(env, rwlock) == 0 {
            return 0; // Successfully grabbed the write lock
        }
        spins += 1;
        if spins > 10 {
            // Safely yield the host thread so the emulator doesn't choke
            std::thread::yield_now(); 
        }
    }
}

pub fn pthread_rwlock_unlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    if rwlock.is_null() { return 22; } // EINVAL
    let state_ptr = rwlock.cast::<u32>();
    let state = env.mem.read(state_ptr);
    
    if state == 0 {
        // Already unlocked, do nothing
    } else if state == 0xFFFFFFFF {
        env.mem.write(state_ptr, 0); // Unlock writer
    } else {
        env.mem.write(state_ptr, state - 1); // Unlock one reader
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
