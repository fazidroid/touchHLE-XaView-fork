/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! POSIX I/O functions (`fcntl.h`, parts of `unistd.h`, etc)

pub mod stat;
pub mod statvfs;

use crate::abi::DotDotDot;
use crate::dyld::{export_c_func, FunctionExports};
use crate::fs::{GuestFile, GuestOpenOptions, GuestPath};
use crate::libc::errno::{set_errno, EBADF, EINTR, EINVAL, EIO, EISDIR, EOVERFLOW, ESPIPE, EAGAIN};
use crate::libc::sys::socket::close_socket;
use crate::libc::unistd::pid_t;
use crate::mem::{
    ConstPtr, ConstVoidPtr, GuestISize, GuestUSize, MutPtr, MutVoidPtr, Ptr, SafeRead,
};
use crate::Environment;
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Default)]
pub struct State {
    /// File descriptors _other than stdin, stdout, and stderr_
    files: Vec<Option<PosixFileHostObject>>,
}
impl State {
    fn file_for_fd(&mut self, fd: FileDescriptor) -> Option<&mut PosixFileHostObject> {
        self.files
            .get_mut(fd_to_file_idx(fd))
            .and_then(|file_or_none| file_or_none.as_mut())
    }
}

pub struct PosixFileHostObject {
    pub file: GuestFile,
    pub needs_flush: bool,
    pub reached_eof: bool,
    pub flags: i32,
}

#[allow(non_camel_case_types)]
pub type FileDescriptor = i32;

fn fd_to_file_idx(fd: FileDescriptor) -> usize {
    assert!(fd >= 3);
    (fd - 3) as usize
}
fn file_idx_to_fd(idx: usize) -> FileDescriptor {
    (idx + 3) as FileDescriptor
}

fn find_or_create_fd(env: &mut Environment, host_object: PosixFileHostObject) -> FileDescriptor {
    let files = &mut env.libc_state.posix_io.files;
    for (idx, file_or_none) in files.iter_mut().enumerate() {
        if file_or_none.is_none() {
            *file_or_none = Some(host_object);
            return file_idx_to_fd(idx);
        }
    }
    let idx = files.len();
    files.push(Some(host_object));
    file_idx_to_fd(idx)
}

fn open(env: &mut Environment, path: ConstPtr<u8>, oflag: i32, _extra: DotDotDot) -> FileDescriptor {
    let path = env.mem.cstr_at_utf8(path).unwrap();
    let guest_path = GuestPath::new(path);
    log_dbg!("open({:?}, {:#x})", guest_path, oflag);

    let options = GuestOpenOptions::from_posix_oflag(oflag);
    let file = match env.fs.open(env, &guest_path, options) {
        Ok(file) => file,
        Err(errno) => {
            set_errno(env, errno);
            return -1;
        }
    };

    let host_object = PosixFileHostObject {
        file,
        needs_flush: false,
        reached_eof: false,
        flags: oflag,
    };
    find_or_create_fd(env, host_object)
}

fn close(env: &mut Environment, fd: FileDescriptor) -> i32 {
    log_dbg!("close({})", fd);
    if fd < 3 {
        // stdin, stdout, stderr
        return 0;
    }
    let idx = fd_to_file_idx(fd);
    let Some(file_or_none) = env.libc_state.posix_io.files.get_mut(idx) else {
        set_errno(env, EBADF);
        return -1;
    };
    let Some(mut host_object) = file_or_none.take() else {
        set_errno(env, EBADF);
        return -1;
    };
    if host_object.needs_flush {
        let _ = host_object.file.flush();
    }
    if let GuestFile::Socket = host_object.file {
        close_socket(env, fd);
    }
    0
}

fn read(env: &mut Environment, fd: FileDescriptor, buf: MutVoidPtr, nbyte: GuestUSize) -> GuestISize {
    if nbyte == 0 {
        return 0;
    }
    let buf = env.mem.ptr_at_mut(buf.cast::<u8>(), nbyte);

    let res = if fd == 0 {
        // stdin
        todo!()
    } else {
        let Some(host_object) = env.libc_state.posix_io.file_for_fd(fd) else {
            set_errno(env, EBADF);
            return -1;
        };
        
        // 🏎️ ASPHALT 6 HACK: Detect if this is a network socket
        let is_socket_file = matches!(host_object.file, GuestFile::Socket);
        
        match host_object.file.read(buf) {
            Ok(n) => {
                // 🏎️ GAMELOFT BYPASS: If a socket returns 0 bytes (EOF), 
                // fake an "Again" error to break the infinite loop!
                if n == 0 && is_socket_file {
                    log!("🏎️ GAMELOFT BYPASS: read() returned 0 on socket {}. Faking EAGAIN to break loop.", fd);
                    set_errno(env, EAGAIN);
                    return -1;
                }
                n as GuestISize
            },
            Err(e) => {
                log!("Warning: read({}, {:?}, {:#x}) failed: {:?}", fd, buf, nbyte, e);
                set_errno(env, EIO);
                -1
            }
        }
    };
    log_dbg!("read({}, {:?}, {:#x}) -> {:#x}", fd, buf, nbyte, res);
    res
}

fn write(env: &mut Environment, fd: FileDescriptor, buf: ConstVoidPtr, nbyte: GuestUSize) -> GuestISize {
    if nbyte == 0 {
        return 0;
    }
    let buf = env.mem.ptr_at(buf.cast::<u8>(), nbyte);

    let res = if fd == 1 || fd == 2 {
        // stdout, stderr
        let s = String::from_utf8_lossy(buf);
        let s = s.strip_suffix('\n').unwrap_or(&s);
        if fd == 1 {
            echo!("{}", s);
        } else {
            log!("{}", s);
        }
        nbyte as GuestISize
    } else {
        let Some(host_object) = env.libc_state.posix_io.file_for_fd(fd) else {
            set_errno(env, EBADF);
            return -1;
        };
        match host_object.file.write(buf) {
            Ok(n) => {
                host_object.needs_flush = true;
                n as GuestISize
            }
            Err(e) => {
                log!("Warning: write({}, {:?}, {:#x}) failed: {:?}", fd, buf, nbyte, e);
                set_errno(env, EIO);
                -1
            }
        }
    };
    log_dbg!("write({}, {:?}, {:#x}) -> {:#x}", fd, buf, nbyte, res);
    res
}

const SEEK_SET: i32 = 0;
const SEEK_CUR: i32 = 1;
const SEEK_END: i32 = 2;

fn lseek(env: &mut Environment, fd: FileDescriptor, offset: i64, whence: i32) -> i64 {
    log_dbg!("lseek({}, {}, {})", fd, offset, whence);
    let Some(host_object) = env.libc_state.posix_io.file_for_fd(fd) else {
        set_errno(env, EBADF);
        return -1;
    };
    let whence = match whence {
        SEEK_SET => SeekFrom::Start(offset as u64),
        SEEK_CUR => SeekFrom::Current(offset),
        SEEK_END => SeekFrom::End(offset),
        _ => {
            set_errno(env, EINVAL);
            return -1;
        }
    };
    match host_object.file.seek(whence) {
        Ok(pos) => pos as i64,
        Err(e) => {
            if matches!(host_object.file, GuestFile::Socket) {
                set_errno(env, ESPIPE);
            } else {
                log!("Warning: lseek({}, {}, {:?}) failed: {:?}", fd, offset, whence, e);
                set_errno(env, EIO);
            }
            -1
        }
    }
}

const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const F_SETLK: i32 = 8;
const F_SETLKW: i32 = 9;

const F_RDLCK: i16 = 1;
const F_UNLCK: i16 = 2;
const F_WRLCK: i16 = 3;

#[repr(C, packed)]
#[allow(non_camel_case_types)]
struct flock {
    start: i64,
    len: i64,
    pid: pid_t,
    lock_type: i16,
    whence: i16,
}
unsafe impl SafeRead for flock {}

fn fcntl(env: &mut Environment, fd: FileDescriptor, cmd: i32, extra: DotDotDot) -> i32 {
    log_dbg!("fcntl({}, {}, ...)", fd, cmd);
    let Some(host_object) = env.libc_state.posix_io.file_for_fd(fd) else {
        set_errno(env, EBADF);
        return -1;
    };
    match cmd {
        F_GETFL => host_object.flags,
        F_SETFL => {
            let flags: i32 = extra.arg(env);
            host_object.flags = flags;
            0
        }
        F_SETLK | F_SETLKW => {
            let lock_ptr: ConstPtr<flock> = extra.arg(env);
            let lock = env.mem.read(lock_ptr);
            if let Err(errno) = validate_lock(env, fd, &lock) {
                set_errno(env, errno);
                return -1;
            }
            0
        }
        // F_NOCACHE
        48 => {
            log!("TODO: Ignoring enabling F_NOCACHE for file descriptor {}", fd);
            0
        }
        _ => {
            log!("Warning: fcntl({}, {}, ...) unimplemented", fd, cmd);
            set_errno(env, EINVAL);
            -1
        }
    }
}

fn fsync(env: &mut Environment, fd: FileDescriptor) -> i32 {
    log_dbg!("fsync({})", fd);
    let Some(host_object) = env.libc_state.posix_io.file_for_fd(fd) else {
        set_errno(env, EBADF);
        return -1;
    };
    match host_object.file.flush() {
        Ok(()) => {
            host_object.needs_flush = false;
            0
        }
        Err(e) => {
            log!("Warning: fsync({}) failed: {:?}", fd, e);
            set_errno(env, EIO);
            -1
        }
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(open(_, _, _)),
    export_c_func!(read(_, _, _)),
    export_c_func!(pread(_, _, _, _)),
    export_c_func!(write(_, _, _)),
    export_c_func!(pwrite(_, _, _, _)),
    export_c_func!(lseek(_, _, _)),
    export_c_func!(close(_)),
    export_c_func!(rename(_, _)),
    export_c_func!(getcwd(_, _)),
    export_c_func!(chdir(_)),
    export_c_func!(fcntl(_, _, _)),
    export_c_func!(flock(_, _)),
    export_c_func!(fsync(_)),
    export_c_func!(ftruncate(_, _)),
];

pub fn register_socket(env: &mut Environment) -> FileDescriptor {
    let host_object = PosixFileHostObject {
        file: GuestFile::Socket,
        needs_flush: false,
        reached_eof: false,
        flags: 0,
    };
    find_or_create_fd(env, host_object)
}

/// Helper function for socket check, not part of API
pub fn is_socket(env: &mut Environment, fd: FileDescriptor) -> bool {
    let guest_file = &env
        .libc_state
        .posix_io
        .files
        .get(fd_to_file_idx(fd))
        .unwrap()
        .as_ref()
        .unwrap()
        .file;
    matches!(guest_file, GuestFile::Socket)
}

/// Helper function to validate lock, not part of API. Assumes fd is a valid
/// file descriptor
fn validate_lock(env: &mut Environment, fd: FileDescriptor, lock: &flock) -> Result<(), i32> {
    let lock_type = lock.lock_type;
    if !matches!(lock_type, F_RDLCK | F_UNLCK | F_WRLCK) {
        return Err(EINVAL);
    }

    let whence = lock.whence as i32;
    let lock_start = match whence {
        SEEK_SET => lock.start,
        SEEK_CUR => {
            let file = env.libc_state.posix_io.file_for_fd(fd).unwrap();
            let file_position = file.file.stream_position().unwrap();
            file_position as i64 + lock.start
        }
        SEEK_END => {
            let file = env.libc_state.posix_io.file_for_fd(fd).unwrap();
            let file_len = file.file.metadata().unwrap().len();
            file_len as i64 + lock.start
        }
        _ => return Err(EINVAL),
    };

    if lock_start < 0 {
        return Err(EINVAL);
    }
    if lock.len < 0 {
        return Err(EOVERFLOW);
    }

    Ok(())
}
