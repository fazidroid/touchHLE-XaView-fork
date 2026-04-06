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
use crate::libc::errno::{set_errno, EBADF, EINVAL, EISDIR, ESPIPE};
use crate::libc::sys::socket::close_socket;
use crate::libc::unistd::pid_t;
use crate::mem::{
    ConstPtr, ConstVoidPtr, GuestISize, GuestUSize, MutPtr, MutVoidPtr, Ptr, SafeRead,
};
use crate::Environment;
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Default)]
pub struct State {
    files: Vec<Option<PosixFileHostObject>>,
}
impl State {
    fn file_for_fd(&mut self, fd: FileDescriptor) -> Option<&mut PosixFileHostObject> {
        self.files
            .get_mut(fd_to_file_idx(fd))
            .and_then(|file_or_none| file_or_none.as_mut())
    }
}

struct PosixFileHostObject {
    file: GuestFile,
    needs_flush: bool,
    reached_eof: bool,
    flags: i32,
}

fn file_idx_to_fd(idx: usize) -> FileDescriptor {
    FileDescriptor::try_from(idx).unwrap() + 3
}
fn fd_to_file_idx(fd: FileDescriptor) -> usize {
    (fd - 3) as usize
}

pub type FileDescriptor = i32;
pub const STDIN_FILENO: FileDescriptor = 0;
pub const STDOUT_FILENO: FileDescriptor = 1;
pub const STDERR_FILENO: FileDescriptor = 2;

pub type OpenFlag = i32;
pub const O_RDONLY: OpenFlag = 0x0;
pub const O_WRONLY: OpenFlag = 0x1;
pub const O_RDWR: OpenFlag = 0x2;
pub const O_ACCMODE: OpenFlag = O_RDWR | O_WRONLY | O_RDONLY;
pub const O_CREAT: OpenFlag = 0x200;
pub const O_TRUNC: OpenFlag = 0x400;

pub type FileControlCommand = i32;
const F_GETFD: FileControlCommand = 1;
const F_SETFD: FileControlCommand = 2;
const F_GETFL: FileControlCommand = 3;
const F_SETFL: FileControlCommand = 4;

#[repr(C, packed)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
struct flock {
    start: i64,
    len: i64,
    pid: i32,
    lock_type: i16,
    whence: i16,
}
unsafe impl SafeRead for flock {}

fn open(env: &mut Environment, path: ConstPtr<u8>, flags: i32, _args: DotDotDot) -> FileDescriptor {
    set_errno(env, 0);
    self::open_direct(env, path, flags)
}

pub fn open_direct(env: &mut Environment, path: ConstPtr<u8>, flags: i32) -> FileDescriptor {
    if path.is_null() {
        set_errno(env, EINVAL);
        return -1;
    }

    let mut needs_flush = false;
    let mut options = GuestOpenOptions::new();
    match flags & O_ACCMODE {
        O_RDONLY => { options.read(); }
        O_WRONLY => { options.write(); needs_flush = true; }
        O_RDWR => { options.read().write(); needs_flush = true; }
        _ => { set_errno(env, EINVAL); return -1; }
    };

    if (flags & O_CREAT) != 0 { options.create(); }
    if (flags & O_TRUNC) != 0 { options.truncate(); }

    let path_string = match env.mem.cstr_at_utf8(path) {
        Ok(s) => s.to_owned(),
        Err(_) => { set_errno(env, EINVAL); return -1; }
    };
    
    match env.fs.open_with_options(GuestPath::new(&path_string), options) {
        Ok(file) => {
            let host_object = PosixFileHostObject {
                file,
                needs_flush,
                reached_eof: false,
                flags: 0,
            };
            find_or_create_fd(env, host_object)
        }
        Err(()) => -1,
    }
}

pub fn read(
    env: &mut Environment,
    fd: FileDescriptor,
    buffer: MutVoidPtr,
    size: GuestUSize,
) -> GuestISize {
    set_errno(env, 0);
    if fd == STDIN_FILENO { return 0; }

    let Some(file) = env.libc_state.posix_io.file_for_fd(fd) else {
        set_errno(env, EBADF);
        return -1;
    };

    let buffer_slice = env.mem.bytes_at_mut(buffer.cast(), size);
    match file.file.read(buffer_slice) {
        Ok(n) => {
            if n == 0 && size != 0 { file.reached_eof = true; }
            n.try_into().unwrap()
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::IsADirectory {
                set_errno(env, EISDIR);
                return 0; // Return EOF for directories to break infinite loops
            }
            -1
        }
    }
}

pub fn write(
    env: &mut Environment,
    fd: FileDescriptor,
    buffer: ConstVoidPtr,
    size: GuestUSize,
) -> GuestISize {
    if fd == STDOUT_FILENO || fd == STDERR_FILENO {
        return size.try_into().unwrap();
    }
    let Some(file) = env.libc_state.posix_io.file_for_fd(fd) else {
        set_errno(env, EBADF);
        return -1;
    };
    let buffer_slice = env.mem.bytes_at(buffer.cast(), size);
    match file.file.write(buffer_slice) {
        Ok(n) => n.try_into().unwrap(),
        Err(_) => -1,
    }
}

pub fn lseek(env: &mut Environment, fd: FileDescriptor, offset: i64, whence: i32) -> i64 {
    let Some(file) = env.libc_state.posix_io.file_for_fd(fd) else {
        set_errno(env, EBADF);
        return -1;
    };

    // FIXED: Prevent Asphalt 6 from trying to rewind a directory and causing a loop
    if matches!(file.file, GuestFile::Directory) {
        return 0; 
    }

    if !file.file.is_seekable() {
        set_errno(env, ESPIPE);
        return -1;
    }
    let seek_pos = match whence {
        0 => SeekFrom::Start(offset as u64),
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => { set_errno(env, EINVAL); return -1; }
    };
    match file.file.seek(seek_pos) {
        Ok(n) => { file.reached_eof = false; n as i64 }
        Err(_) => -1,
    }
}

fn fcntl(env: &mut Environment, fd: FileDescriptor, cmd: FileControlCommand, args: DotDotDot) -> i32 {
    match cmd {
        F_GETFL => 0,
        F_SETFL => 0,
        F_GETFD => {
            if let Some(file) = env.libc_state.posix_io.file_for_fd(fd) { file.flags } else { 0 }
        }
        F_SETFD => {
            let flags: i32 = args.start().next(env);
            if let Some(file) = env.libc_state.posix_io.file_for_fd(fd) { file.flags = flags; }
            0
        }
        _ => 0,
    }
}

fn find_or_create_fd(env: &mut Environment, host_object: PosixFileHostObject) -> FileDescriptor {
    let idx = if let Some(free_idx) = env.libc_state.posix_io.files.iter().position(|f| f.is_none()) {
        env.libc_state.posix_io.files[free_idx] = Some(host_object);
        free_idx
    } else {
        let idx = env.libc_state.posix_io.files.len();
        env.libc_state.posix_io.files.push(Some(host_object));
        idx
    };
    file_idx_to_fd(idx)
}

pub fn close(env: &mut Environment, fd: FileDescriptor) -> i32 {
    if fd < 3 { return 0; }
    if let Some(slot) = env.libc_state.posix_io.files.get_mut(fd_to_file_idx(fd)) {
        *slot = None;
        0
    } else { -1 }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(open(_, _, _)),
    export_c_func!(read(_, _, _)),
    export_c_func!(write(_, _, _)),
    export_c_func!(lseek(_, _, _)),
    export_c_func!(fcntl(_, _, _)),
    export_c_func!(close(_)),
];
