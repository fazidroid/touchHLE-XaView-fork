/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Conditional variables.

use super::mutex::pthread_mutex_t;
use crate::dyld::FunctionExports;
use crate::libc::pthread::mutex::{pthread_mutex_lock, pthread_mutex_unlock};
use crate::mem::{ConstPtr, MutPtr, SafeRead};
use crate::{export_c_func, Environment};
use std::collections::{HashMap, VecDeque};

use crate::environment::{MutexId, ThreadBlock, ThreadId};

#[repr(C, packed)]
pub struct pthread_condattr_t {}
unsafe impl SafeRead for pthread_condattr_t {}

#[repr(C, packed)]
pub struct OpaqueCond {
    _unused: i32,
}
unsafe impl SafeRead for OpaqueCond {}

pub type pthread_cond_t = MutPtr<OpaqueCond>;

#[derive(Default)]
pub struct State {
    pub condition_variables: HashMap<pthread_cond_t, CondHostObject>,
}
impl State {
    fn get(env: &Environment) -> &Self {
        &env.libc_state.pthread.cond
    }
    fn get_mut(env: &mut Environment) -> &mut Self {
        &mut env.libc_state.pthread.cond
    }
}

pub struct CondHostObject {
    waiting: VecDeque<ThreadId>,
    pub(crate) waking: VecDeque<ThreadId>,
    pub(crate) curr_mutex: Option<MutexId>,
    /// Scheduler ticks each waiter has been blocked.
    wait_ticks: VecDeque<u32>,
}

pub fn pthread_cond_init(
    env: &mut Environment,
    cond: MutPtr<pthread_cond_t>,
    attr: ConstPtr<pthread_condattr_t>,
) -> i32 {
    assert!(attr.is_null());
    let opaque = env.mem.alloc_and_write(OpaqueCond { _unused: 0 });
    env.mem.write(cond, opaque);

    assert!(!State::get(env).condition_variables.contains_key(&opaque));
    State::get_mut(env).condition_variables.insert(
        opaque,
        CondHostObject {
            waiting: VecDeque::new(),
            waking: VecDeque::new(),
            curr_mutex: None,
            wait_ticks: VecDeque::new(),
        },
    );
    0 // success
}

pub fn pthread_cond_wait(
    env: &mut Environment,
    cond: MutPtr<pthread_cond_t>,
    mutex: MutPtr<pthread_mutex_t>,
) -> i32 {
    let res = pthread_mutex_unlock(env, mutex);
    assert_eq!(res, 0);
    log_dbg!(
        "Thread {} is blocking on condition variable {:?}",
        env.current_thread,
        cond
    );
    let current_thread = env.current_thread;
    let mutex_id = env.mem.read(mutex).mutex_id;
    let cond_var = env.mem.read(cond);
    let host_object = State::get_mut(env)
        .condition_variables
        .get_mut(&cond_var)
        .unwrap();
    assert!(
        host_object.curr_mutex == Some(mutex_id)
            || host_object.waking.is_empty() && host_object.waiting.is_empty()
    );
    host_object.curr_mutex = Some(mutex_id);
    host_object.waiting.push_back(current_thread);
    host_object.wait_ticks.push_back(0);
    env.yield_thread(ThreadBlock::Condition(cond_var));
    0 // success
}

pub fn pthread_cond_signal(env: &mut Environment, cond: MutPtr<pthread_cond_t>) -> i32 {
    let cond_var = env.mem.read(cond);
    let host_object = State::get_mut(env)
        .condition_variables
        .get_mut(&cond_var)
        .unwrap();
    if let Some(tid) = host_object.waiting.pop_front() {
        host_object.wait_ticks.pop_front();
        host_object.waking.push_back(tid);
        log_dbg!(
            "Thread {} unblocks one thread ({}) waiting on condition variable {:?}",
            env.current_thread,
            tid,
            cond
        );
    } else {
        log_dbg!(
            "Thread {} signals condition variable {:?}, no waiters",
            env.current_thread,
            cond
        );
    }
    0 // success
}

pub fn pthread_cond_broadcast(env: &mut Environment, cond: MutPtr<pthread_cond_t>) -> i32 {
    let cond_var = env.mem.read(cond);
    log_dbg!(
        "Thread {} unblocks one thread waiting on condition variable {:?}",
        env.current_thread,
        cond
    );
    let host_object = State::get_mut(env)
        .condition_variables
        .get_mut(&cond_var)
        .unwrap();
    host_object.waking.extend(host_object.waiting.drain(..));
    host_object.wait_ticks.clear();
    0 // success
}

pub fn pthread_cond_destroy(env: &mut Environment, cond: MutPtr<pthread_cond_t>) -> i32 {
    let cond_var = env.mem.read(cond);
    let old_object = State::get_mut(env)
        .condition_variables
        .remove(&cond_var)
        .unwrap();
    assert!(old_object.waiting.is_empty() && old_object.waking.is_empty());
    env.mem.free(cond_var.cast());
    0 // success
}

pub fn pthread_cond_timedwait(
    env: &mut Environment,
    _cond: MutPtr<pthread_cond_t>,
    mutex: MutPtr<pthread_mutex_t>,
    _abstime: u32,
) -> i32 {
    // GAMELOFT ANTI-FREEZE HACK:
    // touchHLE ignores abstime and sleeps forever. We bypass this by unlocking,
    // relocking, and returning an immediate ETIMEDOUT. This lets the loading 
    // screen progress instead of deadlocking!
    let _ = pthread_mutex_unlock(env, mutex);
    let _ = pthread_mutex_lock(env, mutex);
    60 // Return standard POSIX ETIMEDOUT code
}

/// Maximum scheduler ticks before auto-waking a stuck condvar waiter.
/// Prevents offline-mode deadlocks where a network/store thread waits
/// forever for a server response that never comes.
const MAX_COND_WAIT_TICKS: u32 = 2000;

pub fn tick_cond_watchdog(env: &mut Environment) {
    let mut to_wake: Vec<(pthread_cond_t, ThreadId)> = Vec::new();

    for (cond_var, host_obj) in env.libc_state.pthread.cond.condition_variables.iter_mut() {
        for (idx, ticks) in host_obj.wait_ticks.iter_mut().enumerate() {
            *ticks += 1;
            if *ticks >= MAX_COND_WAIT_TICKS {
                if let Some(&tid) = host_obj.waiting.get(idx) {
                    to_wake.push((*cond_var, tid));
                }
            }
        }
    }

    for (cond_var, tid) in to_wake {
        let host_obj = env.libc_state.pthread.cond
            .condition_variables.get_mut(&cond_var).unwrap();
        if let Some(pos) = host_obj.waiting.iter().position(|&t| t == tid) {
            host_obj.waiting.remove(pos);
            host_obj.wait_ticks.remove(pos);
            host_obj.waking.push_back(tid);
            log!("cond watchdog: auto-waking thread {} on condvar {:?} (no signal after {} ticks)",
                tid, cond_var, MAX_COND_WAIT_TICKS);
        }
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(pthread_cond_init(_, _)),
    export_c_func!(pthread_cond_wait(_, _)),
    export_c_func!(pthread_cond_signal(_)),
    export_c_func!(pthread_cond_broadcast(_)),
    export_c_func!(pthread_cond_destroy(_)),
    export_c_func!(pthread_cond_timedwait(_, _, _)),
];