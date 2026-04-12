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
use crate::libc::errno::{set_errno, EBADF, EINTR, EINVAL, EIO, EISDIR, EOVERFLOW, ESPIPE};
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
    FileDescriptor::try_from(idx)
        .unwrap()
        .checked_add(NORMAL_FILENO_BASE)
        .unwrap()
}
fn fd_to_file_idx(fd: FileDescriptor) -> usize {
    fd.checked_sub(NORMAL_FILENO_BASE).unwrap() as usize
}

pub type FileDescriptor = i32;
pub const STDIN_FILENO: FileDescriptor = 0;
pub const STDOUT_FILENO: FileDescriptor = 1;
pub const STDERR_FILENO: FileDescriptor = 2;
const NORMAL_FILENO_BASE: FileDescriptor = STDERR_FILENO + 1;

pub type OpenFlag = i32;
pub const O_RDONLY: OpenFlag = 0x0;
pub const O_WRONLY: OpenFlag = 0x1;
pub const O_RDWR: OpenFlag = 0x2;
pub const O_ACCMODE: OpenFlag = O_RDWR | O_WRONLY | O_RDONLY;
pub const O_NONBLOCK: OpenFlag = 0x4;
pub const O_APPEND: OpenFlag = 0x8;
pub const O_SHLOCK: OpenFlag = 0x10;
pub const O_NOFOLLOW: OpenFlag = 0x100;
pub const O_CREAT: OpenFlag = 0x200;
pub const O_TRUNC: OpenFlag = 0x400;
pub const O_EXCL: OpenFlag = 0x800;

pub type FileControlCommand = i32;
const F_GETFD: FileControlCommand = 1;
const F_SETFD: FileControlCommand = 2;
const F_GETFL: FileControlCommand = 3;
const F_SETFL: FileControlCommand = 4;
const F_GETLK: FileControlCommand = 7;
const F_SETLK: FileControlCommand = 8;
const F_RDADVISE: FileControlCommand = 44;
const F_NOCACHE: FileControlCommand = 48;

pub type FDFlag = i32;
pub const FD_CLOEXEC: FDFlag = 1;

pub type RecordLockingFlag = i16;
pub const F_RDLCK: RecordLockingFlag = 1;
pub const F_UNLCK: RecordLockingFlag = 2;
pub const F_WRLCK: RecordLockingFlag = 3;

#[repr(C, packed)]
#[derive(Debug)]
#[allow(non_camel_case_types)]
struct flock {
    start: off_t,
    len: off_t,
    pid: pid_t,
    lock_type: i16,
    whence: i16,
}
unsafe impl SafeRead for flock {}

pub type FLockFlag = i32;
pub const LOCK_SH: FLockFlag = 1;
pub const LOCK_EX: FLockFlag = 2;
pub const LOCK_NB: FLockFlag = 4;
pub const LOCK_UN: FLockFlag = 8;

fn open(env: &mut Environment, path: ConstPtr<u8>, flags: i32, _args: DotDotDot) -> FileDescriptor {
    set_errno(env, 0);
    self::open_direct(env, path, flags)
}

pub fn open_direct(env: &mut Environment, path: ConstPtr<u8>, flags: i32) -> FileDescriptor {
    if path.is_null() { return -1; }

    let mut needs_flush = false;
    let mut options = GuestOpenOptions::new();
    match flags & O_ACCMODE {
        O_RDONLY => { options.read(); }
        O_WRONLY => { options.write(); needs_flush = true; }
        O_RDWR => { options.read().write(); needs_flush = true; }
        _ => panic!(),
    };
    if (flags & O_APPEND) != 0 { options.append(); }
    if (flags & O_CREAT) != 0 { options.create(); }
    if (flags & O_TRUNC) != 0 { options.truncate(); }

    let path_string = match env.mem.cstr_at_utf8(path) {
        Ok(path_str) => path_str.to_owned(),
        Err(_) => return -1,
    };

    let res = match env.fs.open_with_options(GuestPath::new(&path_string), options) {
        Ok(file) => {
            let host_object = PosixFileHostObject { file, needs_flush, reached_eof: false, flags: 0 };
            find_or_create_fd(env, host_object)
        }
        Err(()) => -1
    };
    
    if res != -1 && (flags & O_SHLOCK) != 0 { flock(env, res, LOCK_SH); }
    res
}

pub fn read(env: &mut Environment, fd: FileDescriptor, buffer: MutVoidPtr, size: GuestUSize) -> GuestISize {
    set_errno(env, 0);
    if buffer.is_null() { return -1; }
    let Some(file) = env.libc_state.posix_io.file_for_fd(fd) else { return -1; };

    let buffer_slice = env.mem.bytes_at_mut(buffer.cast(), size);
    match file.file.read(buffer_slice) {
        Ok(bytes_read) => {
            if bytes_read == 0 && size != 0 { file.reached_eof = true; }
            bytes_read.try_into().unwrap()
        }
        Err(e) => match e.kind() {
            std::io::ErrorKind::IsADirectory => { set_errno(env, EISDIR); 0 }
            std::io::ErrorKind::WouldBlock => { set_errno(env, crate::libc::errno::EAGAIN); -1 }
            _ => -1
        }
    }
}

pub fn pread(env: &mut Environment, fd: FileDescriptor, buffer: MutVoidPtr, size: GuestUSize, offset: off_t) -> GuestISize {
    let original_position = lseek(env, fd, 0, SEEK_CUR);
    if original_position == -1 { return -1; }
    if lseek(env, fd, offset, SEEK_SET) == -1 { return -1; }
    let bytes_read = read(env, fd, buffer, size);
    assert!(lseek(env, fd, original_position, SEEK_SET) != -1);
    bytes_read
}

pub(super) fn eof(env: &mut Environment, fd: FileDescriptor) -> i32 {
    let file = env.libc_state.posix_io.file_for_fd(fd).unwrap();
    if file.reached_eof { 1 } else { 0 }
}

pub(super) fn clearerr(env: &mut Environment, fd: FileDescriptor) {
    set_errno(env, 0);
    let file = env.libc_state.posix_io.file_for_fd(fd).unwrap();
    file.reached_eof = false;
}

pub(super) fn fflush(env: &mut Environment, fd: FileDescriptor) -> i32 {
    set_errno(env, 0);
    let Some(file) = env.libc_state.posix_io.file_for_fd(fd) else { return -1; };
    match file.file.flush() { Ok(_) => 0, Err(_) => -1, }
}

pub fn write(env: &mut Environment, fd: FileDescriptor, buffer: ConstVoidPtr, size: GuestUSize) -> GuestISize {
    set_errno(env, 0);
    let file = env.libc_state.posix_io.file_for_fd(fd).unwrap();

    let buffer_slice = env.mem.bytes_at(buffer.cast(), size);
    match file.file.write(buffer_slice) {
        Ok(bytes_written) => bytes_written.try_into().unwrap(),
        Err(_) => -1
    }
}

pub fn pwrite(env: &mut Environment, fd: FileDescriptor, buffer: ConstVoidPtr, size: GuestUSize, offset: off_t) -> GuestISize {
    let original_position = lseek(env, fd, 0, SEEK_CUR);
    if original_position == -1 { return -1; }
    if lseek(env, fd, offset, SEEK_SET) == -1 { return -1; }
    let bytes_written = write(env, fd, buffer, size);
    assert!(lseek(env, fd, original_position, SEEK_SET) != -1);
    bytes_written
}

#[allow(non_camel_case_types)]
pub type off_t = i64;
pub const SEEK_SET: i32 = 0;
pub const SEEK_CUR: i32 = 1;
pub const SEEK_END: i32 = 2;
pub fn lseek(env: &mut Environment, fd: FileDescriptor, offset: off_t, whence: i32) -> off_t {
    let Some(file) = env.libc_state.posix_io.file_for_fd(fd) else {
        set_errno(env, EBADF);
        return -1;
    };

    if !file.file.is_seekable() {
        set_errno(env, ESPIPE);
        return -1;
    }

    let start_position = match whence {
        SEEK_SET => 0,
        SEEK_CUR => match file.file.stream_position() {
            Ok(pos) => pos,
            Err(seek_error) => {
                match seek_error.kind() {
                    std::io::ErrorKind::IsADirectory => set_errno(env, EISDIR),
                    _ => unimplemented!("Unexpected seek error {:?}", seek_error),
                }
                return -1;
            }
        },
        SEEK_END => match file.file.stream_len() {
            Ok(len) => len,
            Err(seek_error) => {
                match seek_error.kind() {
                    std::io::ErrorKind::IsADirectory => set_errno(env, EISDIR),
                    _ => unimplemented!("Unexpected seek error {:?}", seek_error),
                }
                return -1;
            }
        },
        _ => {
            set_errno(env, EINVAL);
            return -1;
        }
    };

    let seek_position = match start_position.checked_add_signed(offset) {
        Some(position) => position,
        None => {
            set_errno(env, if offset >= 0 { EOVERFLOW } else { EINVAL });
            return -1;
        }
    };

    if seek_position > off_t::MAX as u64 {
        set_errno(env, EOVERFLOW);
        return -1;
    }

    match file.file.seek(SeekFrom::Start(seek_position)) {
        Ok(new_offset) => {
            file.reached_eof = false;
            new_offset.try_into().unwrap()
        }
        Err(seek_error) => {
            match seek_error.kind() {
                std::io::ErrorKind::InvalidInput => set_errno(env, EINVAL),
                std::io::ErrorKind::IsADirectory => set_errno(env, EISDIR),
                _ => unimplemented!("Unexpected seek error {:?}", seek_error),
            }
            return -1;
        }
    }
}

pub fn close(env: &mut Environment, fd: FileDescriptor) -> i32 {
    set_errno(env, 0);
    if matches!(fd, STDIN_FILENO | STDOUT_FILENO | STDERR_FILENO) { return 0; }

    if fd < 0 || env.libc_state.posix_io.files.get(fd_to_file_idx(fd)).is_none() {
        set_errno(env, EBADF);
        return -1;
    }

    match env.libc_state.posix_io.files[fd_to_file_idx(fd)].take() {
        Some(file) => {
            match file.file {
                GuestFile::Directory => 0,
                GuestFile::Socket => { close_socket(env, fd); 0 }
                _ => {
                    if !file.needs_flush { 0 } else {
                        match file.file.sync_all() { Ok(()) => 0, Err(_) => -1 }
                    }
                }
            }
        }
        None => { set_errno(env, EBADF); -1 }
    }
}

fn rename(env: &mut Environment, old: ConstPtr<u8>, new: ConstPtr<u8>) -> i32 {
    set_errno(env, 0);
    let old = env.mem.cstr_at_utf8(old).unwrap();
    let new = env.mem.cstr_at_utf8(new).unwrap();
    match env.fs.rename(GuestPath::new(&old), GuestPath::new(&new)) { Ok(_) => 0, Err(_) => -1 }
}

pub fn getcwd(env: &mut Environment, buf_ptr: MutPtr<u8>, buf_size: GuestUSize) -> MutPtr<u8> {
    let working_directory = env.fs.working_directory();
    if !env.fs.is_dir(working_directory) { return Ptr::null(); }

    let working_directory = env.fs.working_directory().as_str().as_bytes();

    if buf_ptr.is_null() { return env.mem.alloc_and_write_cstr(working_directory); }

    let res_size: GuestUSize = u32::try_from(working_directory.len()).unwrap() + 1;
    if buf_size < res_size { return Ptr::null(); }

    let buf = env.mem.bytes_at_mut(buf_ptr, res_size);
    buf[..(res_size - 1) as usize].copy_from_slice(working_directory);
    buf[(res_size - 1) as usize] = b'\0';

    buf_ptr
}

fn chdir(env: &mut Environment, path_ptr: ConstPtr<u8>) -> i32 {
    set_errno(env, 0);
    let path = GuestPath::new(env.mem.cstr_at_utf8(path_ptr).unwrap());
    match env.fs.change_working_directory(path) { Ok(_) => 0, Err(()) => -1 }
}

fn fcntl(
    env: &mut Environment,
    fd: FileDescriptor,
    cmd: FileControlCommand,
    args: DotDotDot,
) -> i32 {
    set_errno(env, 0);

    if fd >= NORMAL_FILENO_BASE && env.libc_state.posix_io.files.get(fd_to_file_idx(fd)).is_none() {
        set_errno(env, EBADF);
        return -1;
    }

    match cmd {
        F_GETFD => {
            let file = env.libc_state.posix_io.file_for_fd(fd).unwrap();
            return file.flags;
        }
        F_SETFD => {
            let flags: i32 = args.start().next(env);
            let file = env.libc_state.posix_io.file_for_fd(fd).unwrap();
            file.flags = flags;
        }
        F_GETFL => { return O_RDWR; }
        F_SETFL => { return 0; }
        F_GETLK => {
            let lock_ptr: MutPtr<flock> = args.start().next(env);
            let mut lock = env.mem.read(lock_ptr);
            if let Err(error_code) = validate_lock(env, fd, &lock) { set_errno(env, error_code); return -1; }
            lock.lock_type = F_UNLCK;
            env.mem.write(lock_ptr, lock);
        }
        F_SETLK => {
            let lock_ptr: MutPtr<flock> = args.start().next(env);
            let lock = env.mem.read(lock_ptr);
            if let Err(error_code) = validate_lock(env, fd, &lock) { set_errno(env, error_code); return -1; }
        }
        F_NOCACHE | F_RDADVISE => { }
        _ => { return 0; }
    }
    0
}

fn flock(env: &mut Environment, _fd: FileDescriptor, _operation: FLockFlag) -> i32 {
    set_errno(env, 0);
    0
}

fn fsync(env: &mut Environment, fd: FileDescriptor) -> i32 {
    let Some(file) = env.libc_state.posix_io.file_for_fd(fd) else {
        set_errno(env, EBADF);
        return -1;
    };
    match file.file.sync_all() {
        Ok(()) => 0,
        Err(error) => {
            match error.kind() {
                std::io::ErrorKind::PermissionDenied => return 0,
                std::io::ErrorKind::Unsupported => set_errno(env, EINVAL),
                std::io::ErrorKind::Interrupted => set_errno(env, EINTR),
                _ => set_errno(env, EIO),
            }
            -1
        }
    }
}

fn ftruncate(env: &mut Environment, fd: FileDescriptor, len: off_t) -> i32 {
    set_errno(env, 0);
    let file = env.libc_state.posix_io.file_for_fd(fd).unwrap();
    match file.file.set_len(len as u64) { Ok(()) => 0, Err(_) => -1, }
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

pub fn find_or_create_socket(env: &mut Environment) -> FileDescriptor {
    let host_object = PosixFileHostObject { file: GuestFile::Socket, needs_flush: false, reached_eof: false, flags: 0 };
    find_or_create_fd(env, host_object)
}

pub fn is_socket(env: &mut Environment, fd: FileDescriptor) -> bool {
    let guest_file = &env.libc_state.posix_io.files.get(fd_to_file_idx(fd)).unwrap().as_ref().unwrap().file;
    matches!(guest_file, GuestFile::Socket)
}

fn validate_lock(env: &mut Environment, fd: FileDescriptor, lock: &flock) -> Result<(), i32> {
    let lock_type = lock.lock_type;
    if !matches!(lock_type, F_RDLCK | F_UNLCK | F_WRLCK) { return Err(EINVAL); }

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
            let size: i64 = file.file.stream_len().unwrap().try_into().unwrap();
            size + lock.start
        }
        _ => return Err(EINVAL),
    };

    if lock_start < 0 { return Err(EINVAL); }
    Ok(())
}
