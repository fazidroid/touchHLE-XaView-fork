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

use crate::environment::{ThreadBlock, ThreadId};

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
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(pthread_cond_init(_, _, _)),
    export_c_func!(pthread_cond_signal(_, _)),
    export_c_func!(pthread_cond_broadcast(_, _)),
    export_c_func!(pthread_cond_destroy(_, _)),
    export_c_func!(pthread_cond_wait(_, _, _)),
    export_c_func!(pthread_cond_timedwait(_, _, _, _)),
];

pub fn pthread_cond_init(
    env: &mut Environment,
    cond: MutPtr<pthread_cond_t>,
    _attr: ConstPtr<pthread_condattr_t>,
) -> i32 {
    let cond_var = env.mem.alloc_and_write(OpaqueCond { _unused: 0 }).cast();
    env.mem.write(cond, cond_var);
    State::get_mut(env).condition_variables.insert(
        cond_var,
        CondHostObject {
            waiting: VecDeque::new(),
            waking: VecDeque::new(),
        },
    );
    0
}

pub fn pthread_cond_signal(env: &mut Environment, cond: MutPtr<pthread_cond_t>) -> i32 {
    let cond_var = env.mem.read(cond);
    if let Some(host_object) = State::get_mut(env).condition_variables.get_mut(&cond_var) {
        if let Some(thread) = host_object.waiting.pop_front() {
            host_object.waking.push_back(thread);
        }
    }
    0
}

pub fn pthread_cond_broadcast(env: &mut Environment, cond: MutPtr<pthread_cond_t>) -> i32 {
    let cond_var = env.mem.read(cond);
    if let Some(host_object) = State::get_mut(env).condition_variables.get_mut(&cond_var) {
        host_object.waking.extend(host_object.waiting.drain(..));
    }
    0
}

pub fn pthread_cond_destroy(env: &mut Environment, cond: MutPtr<pthread_cond_t>) -> i32 {
    let cond_var = env.mem.read(cond);
    State::get_mut(env).condition_variables.remove(&cond_var);
    let _ = env.mem.free(cond_var.cast());
    0
}

pub fn pthread_cond_wait(
    env: &mut Environment,
    _cond: MutPtr<pthread_cond_t>,
    mutex: MutPtr<pthread_mutex_t>,
) -> i32 {
    // ANTI-FREEZE HACK FOR GAMELOFT GAMES:
    // Instead of permanently parking the thread and waiting for a signal that 
    // might never arrive (due to bugs in Gameloft's engine or our emulator timing),
    // we turn this into a yielding spinlock!
    
    // 1. Unlock the mutex so the background thread can actually do its work
    pthread_mutex_unlock(env, mutex);
    
    // 2. Force the thread to yield this CPU cycle so we don't freeze the emulator loop
    env.block_thread(ThreadBlock::Yield);
    
    // 3. Relock the mutex immediately. If another thread grabbed it, this call will safely 
    // queue our thread to wait for the mutex lock natively.
    pthread_mutex_lock(env, mutex);
    
    0 // Return success immediately
}

pub fn pthread_cond_timedwait(
    env: &mut Environment,
    cond: MutPtr<pthread_cond_t>,
    mutex: MutPtr<pthread_mutex_t>,
    _abstime: u32,
) -> i32 {
    // Just use our anti-freeze wait implementation to prevent deadlocks
    pthread_cond_wait(env, cond, mutex)
}
