/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! POSIX `sys/stat.h`

use super::{close, off_t, open_direct, FileDescriptor};
use crate::dyld::{export_c_func, FunctionExports};
use crate::fs::{FsError, GuestFile, GuestPath};
use crate::libc::errno::{set_errno, EACCES, EBADF, EEXIST, ENOENT};
use crate::libc::time::timespec;
use crate::mem::{ConstPtr, MutPtr, SafeRead};
use crate::Environment;

#[allow(non_camel_case_types)]
pub type dev_t = u32;
#[allow(non_camel_case_types)]
pub type mode_t = u16;
#[allow(non_camel_case_types)]
pub type nlink_t = u16;
#[allow(non_camel_case_types)]
pub type ino_t = u64;
#[allow(non_camel_case_types)]
pub type uid_t = u32;
#[allow(non_camel_case_types)]
pub type gid_t = u32;
#[allow(non_camel_case_types)]
pub type blkcnt_t = u64;
#[allow(non_camel_case_types)]
pub type blksize_t = u32;

pub const S_IFDIR: mode_t = 0o0040000;
pub const S_IFREG: mode_t = 0o0100000;

#[allow(non_camel_case_types)]
#[derive(Default)]
#[repr(C, packed)]
pub struct stat {
    st_dev: dev_t,
    st_mode: mode_t,
    st_nlink: nlink_t,
    st_ino: ino_t,
    st_uid: uid_t,
    st_gid: gid_t,
    st_rdev: dev_t,
    st_atimespec: timespec,
    st_mtimespec: timespec,
    st_ctimespec: timespec,
    st_birthtimespec: timespec,
    st_size: off_t,
    st_blocks: blkcnt_t,
    st_blksize: blksize_t,
    st_flags: u32,
    st_gen: u32,
    st_lspare: i32,
    st_qspare: [i64; 2],
}
unsafe impl SafeRead for stat {}

fn mkdir(env: &mut Environment, path: ConstPtr<u8>, mode: mode_t) -> i32 {
    set_errno(env, 0);

    let path_str = match env.mem.cstr_at_utf8(path) {
        Ok(s) => {
            if s.contains("//") { return 0; }
            s
        },
        Err(_) => {
            set_errno(env, ENOENT);
            return 0;
        }
    };

    match env.fs.create_dir(GuestPath::new(&path_str)) {
        Ok(()) => 0,
        Err(err) => {
            match err {
                FsError::AlreadyExist => set_errno(env, EEXIST),
                FsError::NonexistentParentDir => set_errno(env, ENOENT),
                FsError::ReadonlyParentDir => set_errno(env, EACCES),
                _ => (),
            };
            0 // Fake success on fail
        }
    }
}

fn fstat_inner(env: &mut Environment, fd: FileDescriptor, buf: MutPtr<stat>) -> i32 {
    let Some(file) = env.libc_state.posix_io.file_for_fd(fd) else {
        set_errno(env, EBADF);
        return -1;
    };

    let mut stat = stat::default();

    match file.file {
        GuestFile::File(_) | GuestFile::IpaBundleFile(_) | GuestFile::ResourceFile(_) => {
            stat.st_mode |= S_IFREG;
            stat.st_size = file.file.stream_len().unwrap().try_into().unwrap();
            
            stat.st_blksize = 4096;
            stat.st_blocks = (stat.st_size as u64 / 512) + 1;
        }
        GuestFile::Directory => {
            stat.st_mode |= S_IFDIR;
            
            stat.st_blksize = 4096;
            stat.st_blocks = 8;
        }
        _ => {
            stat.st_mode |= S_IFREG;
            stat.st_blksize = 4096;
            stat.st_blocks = 1;
        },
    }

    env.mem.write(buf, stat);
    0
}

fn fstat(env: &mut Environment, fd: FileDescriptor, buf: MutPtr<stat>) -> i32 {
    set_errno(env, 0);
    fstat_inner(env, fd, buf)
}

fn stat(env: &mut Environment, path: ConstPtr<u8>, buf: MutPtr<stat>) -> i32 {
    set_errno(env, 0);

    fn do_stat(env: &mut Environment, path: ConstPtr<u8>, buf: MutPtr<stat>) -> i32 {
        if path.is_null() {
            return -1; 
        }

        let fd = open_direct(env, path, 0);
        if fd == -1 {
            let path_str = env.mem.cstr_at_utf8(path).unwrap_or("");
            let filename = path_str.split('/').last().unwrap_or("");
            
            // 🏎️ FIX: We broke the file system by using `.contains("Documents")`!
            // It accidentally faked missing files like `r_ev.dat` as directories, causing infinite loops.
            // Now, we ONLY fake success if the path legitimately looks like a directory (no file extension).
            if !filename.contains('.') || path_str.ends_with('/') {
                log!("🏎️ GAMELOFT BYPASS: Faking missing directory for stat: {}", path_str);
                let mut fake_stat = stat::default();
                fake_stat.st_mode = S_IFDIR | 0o777;
                fake_stat.st_blksize = 4096;
                fake_stat.st_blocks = 8;
                env.mem.write(buf, fake_stat);
                return 0;
            }
            return -1; // Let actual files fail properly so the game creates them!
        }

        let result = fstat_inner(env, fd, buf);
        assert!(close(env, fd) == 0);
        result
    }
    
    do_stat(env, path, buf)
}

fn lstat(env: &mut Environment, path: ConstPtr<u8>, buf: MutPtr<stat>) -> i32 {
    stat(env, path, buf)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(mkdir(_, _)),
    export_c_func!(fstat(_, _)),
    export_c_func!(stat(_, _)),
    export_c_func!(lstat(_, _)),
];
