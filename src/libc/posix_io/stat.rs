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

// enum values sourced from ```man 2 stat```
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
    // TODO: handle errno properly
    set_errno(env, 0);

    // BypassMkdirLoop
    let path_str = match env.mem.cstr_at_utf8(path) {
        Ok(s) => {
            if s.contains("//") {
                return 0;
            }
            s
        }
        Err(_) => {
            set_errno(env, ENOENT);
            return 0;
        }
    };

    // TODO: respect the mode
    match env.fs.create_dir(GuestPath::new(&path_str)) {
        Ok(()) => {
            log_dbg!("mkdir({:?} {:?}, {:#x}) => 0", path, path_str, mode);
            0
        }
                Err(err) => {
            log!("Warning: mkdir... failed with {:?}, faking success", err);
            match err {
                FsError::AlreadyExist => set_errno(env, EEXIST),
                FsError::NonexistentParentDir => set_errno(env, ENOENT),
                FsError::ReadonlyParentDir => set_errno(env, EACCES),
                _ => (),
            };
            // FakeSuccessOnFail
            0 // <--- IT RETURNS SUCCESS ANYWAY!
        }
    }
}

/// Helper for [stat()] and [fstat()] that fills the data in the stat struct
/// Helper for [stat()] and [fstat()] that fills the data in the stat struct
fn fstat_inner(env: &mut Environment, fd: FileDescriptor, buf: MutPtr<stat>) -> i32 {
    let Some(file) = env.libc_state.posix_io.file_for_fd(fd) else {
        set_errno(env, EBADF);
        return -1;
    };

    let mut stat = stat::default();

    match file.file {
        GuestFile::File(_) | GuestFile::IpaBundleFile(_) | GuestFile::ResourceFile(_) => {
            stat.st_mode |= S_IFREG;

            // 1. Obtain file size as a strict unsigned 64-bit integer
            let stream_len: u64 = file.file.stream_len().unwrap_or(0);
            
            // 2. Safely cast to signed i64 for the POSIX st_size requirement
            stat.st_size = stream_len.try_into().unwrap();

            // ==========================================================
            // 🏎️ EA & GAMELOFT BYPASS: Populate missing block sizes!
            // ==========================================================
            stat.st_blksize = 4096;
            
            // 3. Keep it unsigned (u64) for the st_blocks math!
            stat.st_blocks = if stream_len > 0 {
                (stream_len + 511) / 512
            } else {
                8 // Fake a minimum allocation block for newly created files
            };
        }
        GuestFile::Directory => {
            stat.st_mode |= S_IFDIR;
            
            // Give directories a valid block size too!
            stat.st_blksize = 4096;
            stat.st_blocks = 8;
        }
        _ => unimplemented!(),
    }

    env.mem.write(buf, stat);

    0 // success
}

/// Write a synthetic directory entry into `buf`.
/// Used when we know a path is a directory but `open_direct` cannot open it
/// as a file descriptor (e.g. read-only IPA bundle directories like
/// `shaders/`, `gui/`, `xml/`, `levels/` etc.).
fn write_dir_stat(env: &mut Environment, buf: MutPtr<stat>) {
    let mut s = stat::default();
    s.st_mode = S_IFDIR;
    s.st_nlink = 2; // POSIX: at least 2 for any directory
    env.mem.write(buf, s);
}

fn fstat(env: &mut Environment, fd: FileDescriptor, buf: MutPtr<stat>) -> i32 {
    set_errno(env, 0);
    let result = fstat_inner(env, fd, buf);
    log_dbg!("fstat({:?}, {:?}) -> {}", fd, buf, result);
    result
}

/// fstat64: same as fstat on iOS — the stat struct already uses 64-bit fields.
/// NFS Shift 2 (and other EA titles) call fstat64 instead of fstat.
fn fstat64(env: &mut Environment, fd: FileDescriptor, buf: MutPtr<stat>) -> i32 {
    fstat(env, fd, buf)
}

/// stat64: same as stat on iOS — the stat struct already uses 64-bit fields.
fn stat64(env: &mut Environment, path: ConstPtr<u8>, buf: MutPtr<stat>) -> i32 {
    stat(env, path, buf)
}

/// lstat64: same as lstat on iOS.
fn lstat64(env: &mut Environment, path: ConstPtr<u8>, buf: MutPtr<stat>) -> i32 {
    lstat(env, path, buf)
}

fn stat(env: &mut Environment, path: ConstPtr<u8>, buf: MutPtr<stat>) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    fn do_stat(env: &mut Environment, path: ConstPtr<u8>, buf: MutPtr<stat>) -> i32 {
        if path.is_null() {
            return -1; // TODO: Set errno
        }

        let path_str = match env.mem.cstr_at_utf8(path) {
            Ok(s) => s.to_string(),
            Err(_) => return -1,
        };

        if path_str.is_empty() {
            return -1;
        }

        // ── Step 1: try to open as a regular file first ─────────────────────
        // Try file open before anything else. This handles both real files and
        // IPA bundle files. The old hardcoded path-keyword bypass was removed
        // because it returned S_IFDIR for paths like "ghosts/1.bWU=.ghost" which
        // happened to contain substrings like the legitimate directory names,
        // causing the game to treat save-file paths as directories.
        //
        // Pure virtual dot-entries are always safe to report as directories.
        if path_str == "." || path_str == ".." || path_str == "/" {
            write_dir_stat(env, buf);
            return 0;
        }

        let fd = open_direct(env, path, 0);
        if fd != -1 {
            let result = fstat_inner(env, fd, buf);
            assert!(close(env, fd) == 0);
            return result;
        }

        // ── Fast-path: known file extensions are always files, never dirs ──
        // This avoids the create_dir probe for common bundle assets.
        // The probe itself can create spurious directories (fixed below) but
        // the fast-path is both safer and faster.
        let last_component = path_str.rsplit('/').next().unwrap_or(&path_str);
        let file_extensions = [
            ".nib", ".png", ".jpg", ".jpeg", ".plist", ".sb", ".xml",
            ".json", ".mp3", ".m4a", ".caf", ".wav", ".mp4", ".m4v",
            ".pvr", ".atlasc", ".ttf", ".otf", ".ghost", ".dat", ".bin",
            ".lua", ".pak", ".cfg", ".ini", ".txt", ".csv", ".svg",
            ".prefabs", ".scene", ".mat", ".atlas",
            // PowerVR compiled shader cache and compressed variants
            // Real Racing 3 uses .ptc.pvr.z (compressed PVR compiled shaders)
            // stat.rs was treating these as directories causing magenta rendering
            ".ptc", ".z", ".pvr.z", ".ptc.pvr", ".ptc.pvr.z",
            // Other common game asset extensions
            ".fsh", ".vsh", ".glsl", ".vert", ".frag", ".shader",
            ".model", ".mesh", ".anim", ".skel",
            ".ogg", ".aac", ".flac",
            ".zip", ".gz", ".bz2", ".lz4",
        ];
        if file_extensions.iter().any(|ext| {
            last_component.to_ascii_lowercase().ends_with(ext)
        }) {
            log_dbg!("stat: '{}' fast-path as bundle file (known extension)", path_str);
            let mut s = stat::default();
            s.st_mode  = S_IFREG;
            s.st_nlink = 1;
            env.mem.write(buf, s);
            return 0;
        }

        // ── Step 2: open_direct failed — probe whether it is a directory ─────
        // NonDestructiveProbe: use create_dir ONLY to distinguish "existing dir"
        // from "does not exist". If create_dir SUCCEEDS it means the path did
        // NOT previously exist as a directory — immediately remove the spurious
        // directory we just created and return ENOENT.
        // This prevents ghost directories from being stranded at paths the game
        // later wants to create as files (e.g. ghosts/1.bWU=.ghost → EISDIR crash).
        match env.fs.create_dir(GuestPath::new(&path_str)) {
            Ok(()) => {
                // We accidentally created a directory while probing. Undo it.
                let _ = env.fs.remove(GuestPath::new(&path_str));
                log_dbg!("stat: '{}' did not exist (probe-and-remove), ENOENT", path_str);
                set_errno(env, ENOENT);
                -1
            }
            Err(FsError::AlreadyExist) => {
                // Path exists as an existing writable directory.
                log_dbg!("stat: '{}' is an existing writable directory", path_str);
                write_dir_stat(env, buf);
                0
            }
            Err(FsError::ReadonlyParentDir) => {
                // ReadonlyParentDir: the parent is part of the IPA bundle (read-only).
                // This can mean two very different things:
                //
                //   A) The path IS a real bundle directory  → return S_IFDIR
                //      e.g. "published/particles"
                //
                //   B) The path IS a real bundle FILE that open_direct couldn't open
                //      → must return S_IFREG with size=0, NOT S_IFDIR
                //      e.g. "published/particles/fx.bin", "published/data/pursuit_1.prefabs.sb"
                //
                // Without this distinction, stat() returns S_IFDIR for all bundle
                // asset files. The game's asset loader calls stat() first to check
                // existence, sees S_IFDIR, and when it tries fopen() it gets NULL
                // (because open_direct rejects directories). The loading loop hangs.
                //
                // Heuristic: a last path component that contains '.' is a file.
                // Bundle directories never have file extensions.
                let last = path_str.rsplit('/').next().unwrap_or(&path_str);
                if last.contains('.') {
                    // It's a bundle file — report as a regular file so fopen() can open it.
                    log_dbg!("stat: '{}' is a read-only bundle FILE (has extension)", path_str);
                    let mut s = stat::default();
                    s.st_mode  = S_IFREG;
                    s.st_nlink = 1;
                    // st_size left as 0; the game will fopen() and fread() for actual size.
                    env.mem.write(buf, s);
                    0
                } else {
                    // No extension → it is a bundle directory.
                    log_dbg!("stat: '{}' is a read-only bundle directory", path_str);
                    write_dir_stat(env, buf);
                    0
                }
            }
            Err(FsError::NonexistentParentDir) => {
                set_errno(env, ENOENT);
                -1
            }
            Err(_) => {
                set_errno(env, ENOENT);
                -1
            }
        }
    }

    let result = do_stat(env, path, buf);

    log_dbg!(
        "stat({:?} {:?}, {:?}) -> {}",
        path,
        env.mem.cstr_at_utf8(path),
        buf,
        result
    );
    result
}

fn lstat(env: &mut Environment, path: ConstPtr<u8>, buf: MutPtr<stat>) -> i32 {
    log_once!("Warning: lstat() is implemented as stat() (symbolic links are unsupported for now)");
    stat(env, path, buf)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(mkdir(_, _)),
    export_c_func!(fstat(_, _)),
    export_c_func!(fstat64(_, _)),
    export_c_func!(stat(_, _)),
    export_c_func!(stat64(_, _)),
    export_c_func!(lstat(_, _)),
    export_c_func!(lstat64(_, _)),
];
