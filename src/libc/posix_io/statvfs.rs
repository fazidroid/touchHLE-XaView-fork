/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! POSIX `sys/statvfs.h`

use crate::dyld::{export_c_func, FunctionExports};
use crate::libc::errno::set_errno;
use crate::libc::sys::mount::statfs_inner;
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
    let (result, statfs) = statfs_inner(env, path);

    // Detect game bundle IDs that need generous free space
    let main_bundle: crate::objc::id = crate::objc::msg_class![env; NSBundle mainBundle];
    let mut large_fs = false;
    if main_bundle != crate::objc::nil {
        let bundle_id: crate::objc::id = crate::objc::msg![env; main_bundle bundleIdentifier];
        if bundle_id != crate::objc::nil {
            let bundle_str = crate::frameworks::foundation::ns_string::to_rust_string(env, bundle_id);
            large_fs = bundle_str.to_lowercase().contains("asphalt") ||
                       bundle_str.starts_with("com.ea.nfs") ||
                       bundle_str == "com.ea.nfss2.inc" ||
                       bundle_str == "com.ea.nfss2.bv";
        }
    }

    let (f_bsize, f_frsize, f_blocks, f_bfree, f_bavail) = if large_fs {
        // 32 GB fake drive for Asphalt 8 and NFS games
        (4096, 4096, 8388608, 8388608, 8388608)
    } else {
        // Standard touchHLE sandbox for all other games
        (
            statfs.f_iosize.try_into().unwrap(),
            statfs.f_bsize,
            statfs.f_blocks.try_into().unwrap(),
            statfs.f_bfree.try_into().unwrap(),
            statfs.f_bavail.try_into().unwrap(),
        )
    };

    let statvfs = statvfs {
        f_bsize,
        f_frsize,
        f_blocks,
        f_bfree,
        f_bavail,
        f_files: statfs.f_files.try_into().unwrap(),
        f_ffree: statfs.f_ffree.try_into().unwrap(),
        f_favail: statfs.f_ffree.try_into().unwrap(),
        f_fsid: 0,
        f_flag: statfs.f_flags & ST_RDONLY & ST_NOSUID,
        f_namemax: 255,
    };
    env.mem.write(buf, statvfs);
    log!(
        "statvfs({:?} {:?}, {:?}) -> {}",
        path,
        env.mem.cstr_at_utf8(path),
        buf,
        result
    );
    result
}

pub const FUNCTIONS: FunctionExports = &[export_c_func!(statvfs(_, _))];
