/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `sys/mount.h`, file system statistics

use crate::dyld::{export_c_func, FunctionExports};
use crate::libc::dirent::MAXPATHLEN;
use crate::libc::posix_io::stat::uid_t;
use crate::mem::{ConstPtr, MutPtr, SafeRead};
use crate::Environment;

const MFSTYPENAMELEN: usize = 16;

#[allow(non_camel_case_types)]
#[derive(Default, Debug, Copy, Clone)]
#[repr(C, packed)]
pub struct fsid_t {
    pub val: [i32; 2],
}

#[allow(non_camel_case_types)]
#[derive(Debug)]
#[repr(C, packed)]
pub struct statfs {
    pub f_bsize: u32,
    pub f_iosize: i32,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
    pub f_fsid: fsid_t,
    pub f_owner: uid_t,
    pub f_type: u32,
    pub f_flags: u32,
    pub f_fssubtype: u32,
    pub f_fstypename: [u8; MFSTYPENAMELEN],
    pub f_mntonname: [u8; MAXPATHLEN],
    pub f_mntfromname: [u8; MAXPATHLEN],
    pub f_reserved: [u32; 8],
}
unsafe impl SafeRead for statfs {}

/// Returns true if the current app is Asphalt 8 or an NFS game that needs
/// a huge free space report to avoid "insufficient space" alerts.
fn needs_large_fs(env: &Environment) -> bool {
    let main_bundle: crate::objc::id = crate::objc::msg_class![env; NSBundle mainBundle];
    if main_bundle != crate::objc::nil {
        let bundle_id: crate::objc::id = crate::objc::msg![env; main_bundle bundleIdentifier];
        if bundle_id != crate::objc::nil {
            let bundle_str = crate::frameworks::foundation::ns_string::to_rust_string(env, bundle_id);
            return bundle_str.to_lowercase().contains("asphalt") ||
                   bundle_str.starts_with("com.ea.nfs") ||
                   bundle_str == "com.ea.nfss2.inc" ||
                   bundle_str == "com.ea.nfss2.bv";
        }
    }
    false
}

pub fn statfs_inner(env: &mut Environment, _path: ConstPtr<u8>) -> (i32, statfs) {
    // Start with the default iOS 4.3 simulator values
    let mut statfs = statfs {
        f_bsize: 4096,
        f_iosize: 1048576,
        f_blocks: 16567314,
        f_bfree: 12461147,
        f_bavail: 12397147,
        f_files: 16567312,
        f_ffree: 12397147,
        f_fsid: fsid_t {
            val: [234881026, 17],
        },
        f_owner: 0,
        f_type: 17,
        f_flags: 75550720,
        f_fssubtype: 1,
        f_fstypename: [b'\0'; MFSTYPENAMELEN],
        f_mntonname: [b'\0'; MAXPATHLEN],
        f_mntfromname: [b'\0'; MAXPATHLEN],
        f_reserved: [0u32; 8],
    };
    statfs.f_fstypename[..3].copy_from_slice(b"hfs");
    statfs.f_mntonname[..1].copy_from_slice(b"/");
    statfs.f_mntfromname[..12].copy_from_slice(b"/dev/disk0s2");

    // For Asphalt 8 and NFS games, spoof a huge 100 GB filesystem
    if needs_large_fs(env) {
        let blocks_100gb = 100u64 * 1024 * 1024 * 1024 / 4096; // 26,214,400 blocks of 4KB
        statfs.f_blocks = blocks_100gb;
        statfs.f_bfree = blocks_100gb;
        statfs.f_bavail = blocks_100gb;
        statfs.f_files = 10000000; // plenty of inodes
        statfs.f_ffree = 10000000;
    }

    (0, statfs)
}

fn statfs(env: &mut Environment, path: ConstPtr<u8>, buf: MutPtr<statfs>) -> i32 {
    let (ret, statfs) = statfs_inner(env, path);
    env.mem.write(buf, statfs);
    ret
}

pub const FUNCTIONS: FunctionExports = &[export_c_func!(statfs(_, _))];
