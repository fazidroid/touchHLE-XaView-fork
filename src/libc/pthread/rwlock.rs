use crate::Environment;
use crate::mem::{ConstPtr, MutVoidPtr};

pub fn pthread_rwlock_init(env: &mut Environment, rwlock: MutVoidPtr, _attr: MutVoidPtr) -> u32 {
    // .cast() converts the generic pointer to the specific mutex pointer type,
    // and we pass ConstPtr::null() for the missing attribute argument.
    crate::libc::pthread::mutex::pthread_mutex_init(env, rwlock.cast(), ConstPtr::null()) as u32
}

pub fn pthread_rwlock_destroy(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    crate::libc::pthread::mutex::pthread_mutex_destroy(env, rwlock.cast()) as u32
}

pub fn pthread_rwlock_tryrdlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    crate::libc::pthread::mutex::pthread_mutex_trylock(env, rwlock.cast()) as u32
}

pub fn pthread_rwlock_rdlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    crate::libc::pthread::mutex::pthread_mutex_lock(env, rwlock.cast()) as u32
}

pub fn pthread_rwlock_trywrlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    crate::libc::pthread::mutex::pthread_mutex_trylock(env, rwlock.cast()) as u32
}

pub fn pthread_rwlock_wrlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    crate::libc::pthread::mutex::pthread_mutex_lock(env, rwlock.cast()) as u32
}

pub fn pthread_rwlock_unlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
    crate::libc::pthread::mutex::pthread_mutex_unlock(env, rwlock.cast()) as u32
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
