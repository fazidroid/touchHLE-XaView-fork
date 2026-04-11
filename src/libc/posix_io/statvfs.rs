/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! POSIX `sys/statvfs.h`

use crate::dyld::{export_c_func, FunctionExports};
use crate::libc::errno::set_errno;
use crate::mem::{ConstPtr, MutPtr, SafeRead};
use crate::Environment;

#[allow(non_camel_case_types)]
pub type fsblkcnt_t = u32;
#[allow(non_camel_case_types)]
pub type fsfilcnt_t = u32;

pub const ST_RDONLY: u32 = 1;
pub const ST_NOSUID: u32 = 2;

#[allow(non_camel_case_types)]
#[derive(Default)]
#[repr(C, packed)]
pub struct statvfs {
    f_bsize: u32,
    f_frsize: u32,
    f_blocks: fsblkcnt_t,
    f_bfree: fsblkcnt_t,
    f_bavail: fsblkcnt_t,
    f_files: fsfilcnt_t,
    f_ffree: fsfilcnt_t,
    f_favail: fsfilcnt_t,
    f_fsid: u32,
    f_flag: u32,
    f_namemax: u32,
}
unsafe impl SafeRead for statvfs {}

fn statvfs(env: &mut Environment, path: ConstPtr<u8>, buf: MutPtr<statvfs>) -> i32 {
    set_errno(env, 0);
    
    // 🏎️ GAMELOFT BYPASS: Hardcode ~130GB of free space to bypass storage checks!
    let statvfs_data = statvfs {
        f_bsize: 4096,
        f_frsize: 4096,
        f_blocks: 0x01FFFFFF,
        f_bfree: 0x01FFFFFF,
        f_bavail: 0x01FFFFFF,
        f_files: 0x01FFFFFF,
        f_ffree: 0x01FFFFFF,
        f_favail: 0x01FFFFFF,
        f_fsid: 0,
        f_flag: 0,
        f_namemax: 255,
    };
    env.mem.write(buf, statvfs_data);
    
    log!(
        "🏎️ GAMELOFT BYPASS: Faked massive free space for statvfs({:?})",
        env.mem.cstr_at_utf8(path)
    );
    0 // Success
}

pub const FUNCTIONS: FunctionExports = &[export_c_func!(statvfs(_, _))];
