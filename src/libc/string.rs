/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `string.h`

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstPtr, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr, Ptr};
use crate::Environment;
use std::cmp::Ordering;

use super::generic_char::GenericChar;

#[derive(Default)]
pub struct State {
    strtok: Option<MutPtr<u8>>,
}

fn strtok(env: &mut Environment, s: MutPtr<u8>, sep: ConstPtr<u8>) -> MutPtr<u8> {
    let s = if s.is_null() {
        let state = env.libc_state.string.strtok.unwrap();
        if state.is_null() {
            env.libc_state.string.strtok = None;
            return Ptr::null();
        }
        state
    } else {
        s
    };

    let sep = env.mem.cstr_at(sep);

    let mut token_start = s;
    loop {
        let c = env.mem.read(token_start);
        if c == b'\0' {
            env.libc_state.string.strtok = None;
            return Ptr::null();
        } else if sep.contains(&c) {
            token_start += 1;
        } else {
            break;
        }
    }

    let mut token_end = token_start;
    let next_token = loop {
        let c = env.mem.read(token_end);
        if sep.contains(&c) {
            env.mem.write(token_end, b'\0');
            break token_end + 1;
        } else if c == b'\0' {
            break Ptr::null();
        } else {
            token_end += 1;
        }
    };

    env.libc_state.string.strtok = Some(next_token);

    token_start
}

// Functions shared with wchar.rs

fn bzero(env: &mut Environment, dest: MutVoidPtr, count: GuestUSize) {
    memset(env, dest, 0, count);
}
fn memset(env: &mut Environment, dest: MutVoidPtr, ch: i32, count: GuestUSize) -> MutVoidPtr {
    GenericChar::<u8>::memset(env, dest.cast(), ch as u8, count, GuestUSize::MAX).cast()
}
fn __memset_chk(
    env: &mut Environment,
    dest: MutVoidPtr,
    ch: i32,
    count: GuestUSize,
    dest_count: GuestUSize,
) -> MutVoidPtr {
    GenericChar::<u8>::memset(env, dest.cast(), ch as u8, count, dest_count).cast()
}
fn memset_pattern4(env: &mut Environment, b: MutVoidPtr, pattern4: ConstVoidPtr, len: GuestUSize) {
    memset_pattern_inner(env, b, pattern4, len, 4)
}
fn memset_pattern8(env: &mut Environment, b: MutVoidPtr, pattern8: ConstVoidPtr, len: GuestUSize) {
    memset_pattern_inner(env, b, pattern8, len, 8)
}
fn memset_pattern16(
    env: &mut Environment,
    b: MutVoidPtr,
    pattern16: ConstVoidPtr,
    len: GuestUSize,
) {
    memset_pattern_inner(env, b, pattern16, len, 16)
}
fn memset_pattern_inner(
    env: &mut Environment,
    b: MutVoidPtr,
    pattern: ConstVoidPtr,
    len: GuestUSize,
    pattern_len: GuestUSize,
) {
    assert!(matches!(pattern_len, 4 | 8 | 16));
    let mut tmp = [0; 16];
    tmp[..pattern_len as usize].copy_from_slice(env.mem.bytes_at(pattern.cast(), pattern_len));
    let mut target: MutPtr<u8> = b.cast();
    for _ in 0..(len / pattern_len) {
        env.mem
            .bytes_at_mut(target, pattern_len)
            .copy_from_slice(&tmp[..pattern_len as usize]);
        target += pattern_len;
    }
    for i in 0..(len % pattern_len) {
        let value = env.mem.read(pattern.cast() + i);
        env.mem.write(target, value);
        target += 1;
    }
}
fn memcpy(
    env: &mut Environment,
    dest: MutVoidPtr,
    src: ConstVoidPtr,
    size: GuestUSize,
) -> MutVoidPtr {
    GenericChar::<u8>::memcpy(env, dest.cast(), src.cast(), size, GuestUSize::MAX).cast()
}
fn __memcpy_chk(
    env: &mut Environment,
    dest: MutVoidPtr,
    src: ConstVoidPtr,
    size: GuestUSize,
    dest_size: GuestUSize,
) -> MutVoidPtr {
    GenericChar::<u8>::memcpy(env, dest.cast(), src.cast(), size, dest_size).cast()
}
fn memmove(
    env: &mut Environment,
    dest: MutVoidPtr,
    src: ConstVoidPtr,
    size: GuestUSize,
) -> MutVoidPtr {
    GenericChar::<u8>::memmove(env, dest.cast(), src.cast(), size, GuestUSize::MAX).cast()
}
fn __memmove_chk(
    env: &mut Environment,
    dest: MutVoidPtr,
    src: ConstVoidPtr,
    size: GuestUSize,
    dest_size: GuestUSize,
) -> MutVoidPtr {
    GenericChar::<u8>::memmove(env, dest.cast(), src.cast(), size, dest_size).cast()
}
fn memchr(env: &mut Environment, string: ConstVoidPtr, c: i32, size: GuestUSize) -> ConstVoidPtr {
    GenericChar::<u8>::memchr(env, string.cast(), c as u8, size).cast()
}
fn memcmp(env: &mut Environment, a: ConstVoidPtr, b: ConstVoidPtr, size: GuestUSize) -> i32 {
    GenericChar::<u8>::memcmp(env, a.cast(), b.cast(), size)
}
pub(super) fn strlen(env: &mut Environment, s: ConstPtr<u8>) -> GuestUSize {
    GenericChar::<u8>::strlen(env, s)
}
pub(super) fn strcpy(env: &mut Environment, dest: MutPtr<u8>, src: ConstPtr<u8>) -> MutPtr<u8> {
    GenericChar::<u8>::strcpy(env, dest, src, GuestUSize::MAX)
}
fn __strcpy_chk(
    env: &mut Environment,
    dest: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
) -> MutPtr<u8> {
    GenericChar::<u8>::strcpy(env, dest, src, size)
}
fn strcat(env: &mut Environment, dest: MutPtr<u8>, src: ConstPtr<u8>) -> MutPtr<u8> {
    GenericChar::<u8>::strcat(env, dest, src, GuestUSize::MAX)
}
fn __strcat_chk(
    env: &mut Environment,
    dest: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
) -> MutPtr<u8> {
    GenericChar::<u8>::strcat(env, dest, src, size)
}
fn strcspn(env: &mut Environment, s: ConstPtr<u8>, charset: ConstPtr<u8>) -> GuestUSize {
    GenericChar::<u8>::strcspn(env, s, charset)
}
pub(super) fn strncpy(
    env: &mut Environment,
    dest: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
) -> MutPtr<u8> {
    GenericChar::<u8>::strncpy(env, dest, src, size, GuestUSize::MAX)
}
fn __strncpy_chk(
    env: &mut Environment,
    dest: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
    dest_size: GuestUSize,
) -> MutPtr<u8> {
    GenericChar::<u8>::strncpy(env, dest, src, size, dest_size)
}
fn strsep(env: &mut Environment, stringp: MutPtr<MutPtr<u8>>, delim: ConstPtr<u8>) -> MutPtr<u8> {
    let orig = env.mem.read(stringp);
    if orig.is_null() {
        return Ptr::null();
    }
    let tmp = orig;
    let mut i = 0;
    loop {
        let c = env.mem.read(tmp + i);
        if c == b'\0' {
            env.mem.write(stringp, Ptr::null());
            break;
        }
        let mut j = 0;
        loop {
            let cc = env.mem.read(delim + j);
            if c == cc {
                env.mem.write(tmp + i, b'\0');
                env.mem.write(stringp, tmp + i + 1);
                return orig;
            }
            if cc == b'\0' {
                break;
            }
            j += 1;
        }
        i += 1;
    }
    orig
}
pub(super) fn strdup(env: &mut Environment, src: ConstPtr<u8>) -> MutPtr<u8> {
    GenericChar::<u8>::strdup(env, src)
}
pub fn strcmp(env: &mut Environment, a: ConstPtr<u8>, b: ConstPtr<u8>) -> i32 {
    GenericChar::<u8>::strcmp(env, a, b)
}
fn strncmp(env: &mut Environment, a: ConstPtr<u8>, b: ConstPtr<u8>, n: GuestUSize) -> i32 {
    GenericChar::<u8>::strncmp(env, a, b, n)
}
fn strcasecmp(env: &mut Environment, a: ConstPtr<u8>, b: ConstPtr<u8>) -> i32 {
    // TODO: generalize to wide chars
    let mut offset = 0;
    loop {
        let char_a = env.mem.read(a + offset).to_ascii_lowercase();
        let char_b = env.mem.read(b + offset).to_ascii_lowercase();
        offset += 1;

        match char_a.cmp(&char_b) {
            Ordering::Less => return -1,
            Ordering::Greater => return 1,
            Ordering::Equal => {
                if char_a == u8::default() {
                    return 0;
                } else {
                    continue;
                }
            }
        }
    }
}
fn strncasecmp(env: &mut Environment, a: ConstPtr<u8>, b: ConstPtr<u8>, n: GuestUSize) -> i32 {
    // TODO: generalize to wide chars
    if n == 0 {
        return 0;
    }

    let mut offset = 0;
    loop {
        let char_a = env.mem.read(a + offset).to_ascii_lowercase();
        let char_b = env.mem.read(b + offset).to_ascii_lowercase();
        offset += 1;

        match char_a.cmp(&char_b) {
            Ordering::Less => return -1,
            Ordering::Greater => return 1,
            Ordering::Equal => {
                if offset == n || char_a == u8::default() {
                    return 0;
                } else {
                    continue;
                }
            }
        }
    }
}
fn strncat(env: &mut Environment, s1: MutPtr<u8>, s2: ConstPtr<u8>, n: GuestUSize) -> MutPtr<u8> {
    GenericChar::<u8>::strncat(env, s1, s2, n)
}
fn strstr(env: &mut Environment, string: ConstPtr<u8>, substring: ConstPtr<u8>) -> ConstPtr<u8> {
    GenericChar::<u8>::strstr(env, string, substring)
}
fn strchr(env: &mut Environment, path: ConstPtr<u8>, c: u8) -> ConstPtr<u8> {
    GenericChar::<u8>::strchr(env, path, c)
}
fn strrchr(env: &mut Environment, path: ConstPtr<u8>, c: u8) -> ConstPtr<u8> {
    GenericChar::<u8>::strrchr(env, path, c)
}
fn strlcpy(
    env: &mut Environment,
    dst: MutPtr<u8>,
    src: ConstPtr<u8>,
    size: GuestUSize,
) -> GuestUSize {
    GenericChar::<u8>::strlcpy(env, dst, src, size)
}
fn ___strncat_chk(
    env: &mut Environment,
    dest: MutPtr<u8>,
    src: ConstPtr<u8>,
    n: GuestUSize,
    dest_len: GuestUSize,
) -> MutPtr<u8> {
    let current_dest_len = strlen(env, dest.cast_const());
    let src_len = strlen(env, src);
    let to_copy = n.min(src_len);

    // Verify we aren't about to blow past the buffer size
    if current_dest_len + to_copy >= dest_len {
        panic!("🛡️ SAFETY TRIGGER: ___strncat_chk detected a buffer overflow attempt!");
    }

    // 🛠️ FIX: Convert to usize for Rust indexing
    let to_copy_usize = to_copy as usize;

    let dest_slice = env.mem.bytes_at_mut(dest + current_dest_len, to_copy + 1);
    let src_slice = env.mem.bytes_at(src, to_copy);

    dest_slice[..to_copy_usize].copy_from_slice(src_slice);
    dest_slice[to_copy_usize] = b'\0'; // Ensure it's null-terminated

    dest
}

fn _strspn(env: &mut Environment, s: ConstPtr<u8>, accept: ConstPtr<u8>) -> GuestUSize {
    let s_slice = env.mem.cstr_at(s);
    let accept_slice = env.mem.cstr_at(accept);
    
    let mut count = 0;
    for &byte in s_slice {
        if accept_slice.contains(&byte) {
            count += 1;
        } else {
            break;
        }
    }
    count as GuestUSize
}

fn strpbrk(env: &mut Environment, s: ConstPtr<u8>, charset: ConstPtr<u8>) -> ConstPtr<u8> {
    if s.is_null() || charset.is_null() {
        return Ptr::null();
    }
    let sep = env.mem.cstr_at(charset);
    let mut i = 0;
    loop {
        let c = env.mem.read(s + i);
        if c == b'\0' {
            return Ptr::null();
        }
        if sep.contains(&c) {
            return s + i;
        }
        i += 1;
    }
}

// ЗАГЛУШКА ДЛЯ GAMELOFT LIVE
#[allow(clippy::too_many_arguments)]
fn CCCrypt(
    _env: &mut Environment,
    _op: u32,
    _alg: u32,
    _options: u32,
    _key: ConstVoidPtr,
    _key_len: GuestUSize,
    _iv: ConstVoidPtr,
    _va_args: crate::abi::DotDotDot,
) -> i32 {
    -43 // kCCParamError
}

pub fn strerror(env: &mut Environment, errnum: i32) -> ConstPtr<u8> {
    let msg = format!("Error {}\0", errnum);
    env.mem.alloc_and_write_cstr(msg.as_bytes()).cast_const()
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(strerror(_)),
    export_c_func!(strtok(_, _)),
    export_c_func!(bzero(_, _)),
    // Functions shared with wchar.rs
    export_c_func!(memset(_, _, _)),
    export_c_func!(__memset_chk(_, _, _, _)),
    export_c_func!(memset_pattern4(_, _, _)),
    export_c_func!(memset_pattern8(_, _, _)),
    export_c_func!(memset_pattern16(_, _, _)),
    export_c_func!(memcpy(_, _, _)),
    export_c_func!(__memcpy_chk(_, _, _, _)),
    export_c_func!(memmove(_, _, _)),
    export_c_func!(__memmove_chk(_, _, _, _)),
    export_c_func!(memchr(_, _, _)),
    export_c_func!(memcmp(_, _, _)),
    export_c_func!(strlen(_)),
    export_c_func!(strcpy(_, _)),
    export_c_func!(__strcpy_chk(_, _, _)),
    export_c_func!(strcat(_, _)),
    export_c_func!(strcspn(_, _)),
    export_c_func!(__strcat_chk(_, _, _)),
    export_c_func!(strncpy(_, _, _)),
    export_c_func!(__strncpy_chk(_, _, _, _)),
    export_c_func!(___strncat_chk(_, _, _, _)),
    export_c_func!(_strspn(_, _)),
    export_c_func!(strsep(_, _)),
    export_c_func!(strdup(_)),
    export_c_func!(strcmp(_, _)),
    export_c_func!(strncmp(_, _, _)),
    export_c_func!(strcasecmp(_, _)),
    export_c_func!(strncasecmp(_, _, _)),
    export_c_func!(strncat(_, _, _)),
    export_c_func!(strstr(_, _)),
    export_c_func!(strchr(_, _)),
    export_c_func!(strrchr(_, _)),
    export_c_func!(strlcpy(_, _, _)),
    export_c_func!(strpbrk(_, _)),
    export_c_func!(CCCrypt(_, _, _, _, _, _, _)), // Ровно 7 подчеркиваний!
];
