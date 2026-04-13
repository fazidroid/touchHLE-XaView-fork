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
pub const S_IFSOCK: mode_t = 0o0140000;

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

/// Fill `buf` with a synthetic directory stat. Used when we know a path is a
/// directory but cannot open it as a file descriptor (e.g. IPA bundle dirs).
fn write_dir_stat(env: &mut Environment, buf: MutPtr<stat>) {
    let mut s = stat::default();
    s.st_mode   = S_IFDIR;
    s.st_nlink  = 2;
    s.st_blksize = 4096;
    s.st_blocks  = 8;
    env.mem.write(buf, s);
}

fn mkdir(env: &mut Environment, path: ConstPtr<u8>, _mode: mode_t) -> i32 {
    set_errno(env, 0);

    let path_str = match env.mem.cstr_at_utf8(path) {
        Ok(s) => {
            if s.contains("//") { return 0; }
            if s.is_empty() { return 0; }
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
                FsError::AlreadyExist       => set_errno(env, EEXIST),
                FsError::NonexistentParentDir => set_errno(env, ENOENT),
                FsError::ReadonlyParentDir  => set_errno(env, EACCES),
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

    let mut s = stat::default();

    match file.file {
        GuestFile::File(_) | GuestFile::IpaBundleFile(_) | GuestFile::ResourceFile(_) => {
            s.st_mode |= S_IFREG;
            if let Ok(len) = file.file.stream_len() {
                s.st_size = len.try_into().unwrap_or(0);
            }
            s.st_blksize = 4096;
            s.st_blocks  = (s.st_size as u64 / 512) + 1;
        }
        GuestFile::Directory => {
            s.st_mode   |= S_IFDIR;
            s.st_nlink   = 2;
            s.st_blksize = 4096;
            s.st_blocks  = 8;
        }
        GuestFile::Socket => {
            s.st_mode   |= S_IFSOCK;
            s.st_blksize = 4096;
            s.st_blocks  = 1;
        }
        _ => {
            s.st_mode   |= S_IFREG;
            s.st_blksize = 4096;
            s.st_blocks  = 1;
        }
    }

    env.mem.write(buf, s);
    0
}

fn fstat(env: &mut Environment, fd: FileDescriptor, buf: MutPtr<stat>) -> i32 {
    set_errno(env, 0);
    fstat_inner(env, fd, buf)
}

fn stat(env: &mut Environment, path: ConstPtr<u8>, buf: MutPtr<stat>) -> i32 {
    set_errno(env, 0);

    if path.is_null() {
        set_errno(env, ENOENT);
        return -1;
    }

    // ── Step 1: try to open as a regular file/directory fd ──────────────────
    // open_direct handles regular files and any directory the VFS can open as
    // a fd. If it succeeds we get correct st_mode from fstat_inner.
    let fd = open_direct(env, path, 0);
    if fd != -1 {
        let result = fstat_inner(env, fd, buf);
        assert!(close(env, fd) == 0);
        return result;
    }

    // ── Step 2: open_direct failed — probe whether it is a directory ─────────
    // open_direct only opens files; directories inside the IPA bundle (shaders/,
    // gui/, xml/, levels/, etc.) cannot be opened as fds but DO exist.
    // We probe by attempting create_dir and interpreting the error:
    //
    //   AlreadyExist       → directory exists in a writable area
    //   ReadonlyParentDir  → parent is the read-only IPA bundle mount; the path
    //                        is inside the bundle. We treat it as a directory
    //                        because the game only calls stat() on paths that
    //                        are expected to exist.
    //   NonexistentParentDir → parent doesn't exist at all → ENOENT
    //   other errors         → genuine error → ENOENT

    let path_str = match env.mem.cstr_at_utf8(path) {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_errno(env, ENOENT);
            return -1;
        }
    };

    // Skip obviously empty or degenerate paths.
    if path_str.is_empty() {
        set_errno(env, ENOENT);
        return -1;
    }

    match env.fs.create_dir(GuestPath::new(&path_str)) {
        Ok(()) => {
            // We just created it — shouldn't normally happen during stat, but
            // treat it as a directory since that is what the caller expects.
            log_dbg!("stat: created dir '{}' as side-effect of directory probe", path_str);
            write_dir_stat(env, buf);
            0
        }
        Err(FsError::AlreadyExist) => {
            // Directory already exists in a writable area.
            log_dbg!("stat: '{}' is an existing directory (AlreadyExist)", path_str);
            write_dir_stat(env, buf);
            0
        }
        Err(FsError::ReadonlyParentDir) => {
            // The parent is read-only — this is the IPA bundle mount point.
            // The path (e.g. "shaders", "gui") is a directory inside the bundle.
            // Return a synthetic directory stat so the game's DirStreamFactory
            // does not report "Cannot find host/file/directory".
            log_dbg!("stat: '{}' is a read-only bundle directory (ReadonlyParentDir)", path_str);
            write_dir_stat(env, buf);
            0
        }
        Err(FsError::NonexistentParentDir) => {
            // The parent itself doesn't exist — path is genuinely missing.
            set_errno(env, ENOENT);
            -1
        }
        Err(_) => {
            set_errno(env, ENOENT);
            -1
        }
    }
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