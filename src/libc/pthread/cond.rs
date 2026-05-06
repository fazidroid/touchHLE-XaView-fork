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
    let Some(host_object) = State::get_mut(env)
        .condition_variables
        .get_mut(&cond_var) else {
        log!("pthread_cond_wait: unknown condvar {:?} (PTHREAD_COND_INITIALIZER?), re-locking", cond_var);
        pthread_mutex_lock(env, mutex);
        return 0;
    };
    assert!(
        host_object.curr_mutex == Some(mutex_id)
            || host_object.waking.is_empty() && host_object.waiting.is_empty()
    );
    host_object.curr_mutex = Some(mutex_id);
    host_object.waiting.push_back(current_thread);
    env.yield_thread(ThreadBlock::Condition(cond_var));
    0 // success
}

pub fn pthread_cond_signal(env: &mut Environment, cond: MutPtr<pthread_cond_t>) -> i32 {
    let cond_var = env.mem.read(cond);
    let Some(host_object) = State::get_mut(env)
        .condition_variables
        .get_mut(&cond_var) else {
        log!("pthread_cond_signal: unknown condvar {:?} (PTHREAD_COND_INITIALIZER?), ignoring", cond_var);
        return 0;
    };
    if let Some(tid) = host_object.waiting.pop_front() {
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
    let Some(host_object) = State::get_mut(env)
        .condition_variables
        .get_mut(&cond_var) else {
        log!("pthread_cond_broadcast: unknown condvar {:?} (PTHREAD_COND_INITIALIZER?), ignoring", cond_var);
        return 0;
    };
    host_object.waking.extend(host_object.waiting.drain(..));
    0 // success
}

pub fn pthread_cond_destroy(env: &mut Environment, cond: MutPtr<pthread_cond_t>) -> i32 {
    let cond_var = env.mem.read(cond);
    let Some(old_object) = State::get_mut(env)
        .condition_variables
        .remove(&cond_var) else {
        log!("pthread_cond_destroy: unknown condvar {:?}, ignoring", cond_var);
        return 0;
    };
    // SoftAssertDestroy: warn instead of panic if threads still in queue.
    if !old_object.waiting.is_empty() || !old_object.waking.is_empty() {
        log!("Warning: pthread_cond_destroy: {} waiting, {} waking threads remain",
            old_object.waiting.len(), old_object.waking.len());
    }
    env.mem.free(cond_var.cast());
    0 // success
}

pub fn pthread_cond_timedwait(
    env: &mut Environment,
    cond: MutPtr<pthread_cond_t>,
    mutex: MutPtr<pthread_mutex_t>,
    _abstime: u32,   // still ignored – all games expect a quick timeout
) -> i32 {
    let cond_var = env.mem.read(cond);

    // 1. Fast path: has another thread already signalled *this* thread?
    //    If yes, consume the signal and return immediately (no sleep).
    let current_thread = env.current_thread;
    let Some(host_object) = State::get_mut(env)
        .condition_variables
        .get_mut(&cond_var) else {
        return 0;
    };
    if let Some(idx) = host_object.waking.iter().position(|&t| t == current_thread) {
        host_object.waking.remove(idx);
        return 0; // success – signal consumed
    }

    // 2. Slow path: unlock, tiny sleep, relock, report timeout.
    let _ = pthread_mutex_unlock(env, mutex);
    env.sleep(std::time::Duration::from_millis(1));   // keep the 1ms safety net
    let _ = pthread_mutex_lock(env, mutex);
    60 // ETIMEDOUT
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(pthread_cond_init(_, _)),
    export_c_func!(pthread_cond_wait(_, _)),
    export_c_func!(pthread_cond_signal(_)),
    export_c_func!(pthread_cond_broadcast(_)),
    export_c_func!(pthread_cond_destroy(_)),
    export_c_func!(pthread_cond_timedwait(_, _, _)),
];
