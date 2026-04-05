/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! POSIX Threads implementation.
//!
//! The pthread API often wants functions to check some precondition and return
//! an error if it isn't met. For convenience and for the sake of debugging this
//! implementation, we'll usually assert on these conditions instead, assuming
//! that the app is well-written and that it won't rely on these soft failures.
//! Cases like this will be marked with a comment saying what error should have
//! been returned, e.g. `assert!(...); // should be EINVAL`.

#![allow(non_camel_case_types)]

/// Helper macro for the common pattern of checking magic numbers and returning
/// [crate::libc::errno::EINVAL] on failure.
///
/// Usage: `check_magic!(env, some_ptr, 0xABAD1DEA);`
macro_rules! check_magic {
    ($env:ident, $object:ident, $expected:ident) => {
        let actual = $env.mem.read($object.cast::<u32>());
        if actual != $expected {
            log!("Warning: failed magic number check for pthread object at {:?}: expected {:#x}, got {:#x}", $object, $expected, actual);
            return $crate::libc::errno::EINVAL;
        }
    }
}

pub mod cond;
pub mod key;
pub mod mutex;
pub mod once;
pub mod thread;

// --- RWLOCK IMPLEMENTATION FOR ANDROID LOAD FREEZE ---
pub mod rwlock {
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
            return 16; // EBUSY
        }
        env.mem.write(state_ptr, state + 1);
        0
    }

    pub fn pthread_rwlock_rdlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
        pthread_rwlock_tryrdlock(env, rwlock)
    }

    pub fn pthread_rwlock_trywrlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
        if rwlock.is_null() { return 22; } // EINVAL
        let state_ptr = rwlock.cast::<u32>();
        let state = env.mem.read(state_ptr);
        if state != 0 {
            return 16; // EBUSY
        }
        env.mem.write(state_ptr, 0xFFFFFFFF);
        0
    }

    pub fn pthread_rwlock_wrlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
        pthread_rwlock_trywrlock(env, rwlock)
    }

    pub fn pthread_rwlock_unlock(env: &mut Environment, rwlock: MutVoidPtr) -> u32 {
        if rwlock.is_null() { return 22; } // EINVAL
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

    // THIS EXPORTS THE FUNCTIONS TO LIBC.RS
    crate::export_guest_funcs! {
        "pthread_rwlock_init" => pthread_rwlock_init,
        "pthread_rwlock_destroy" => pthread_rwlock_destroy,
        "pthread_rwlock_tryrdlock" => pthread_rwlock_tryrdlock,
        "pthread_rwlock_rdlock" => pthread_rwlock_rdlock,
        "pthread_rwlock_trywrlock" => pthread_rwlock_trywrlock,
        "pthread_rwlock_wrlock" => pthread_rwlock_wrlock,
        "pthread_rwlock_unlock" => pthread_rwlock_unlock,
    }
}
// -----------------------------------------------------

#[derive(Default)]
pub struct State {
    pub cond: cond::State,
    key: key::State,
    thread: thread::State,
}
