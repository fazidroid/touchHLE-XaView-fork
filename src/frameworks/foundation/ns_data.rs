/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSData` and `NSMutableData`.

use super::ns_string::to_rust_string;
use super::{NSRange, NSUInteger};
use crate::frameworks::foundation::ns_keyed_unarchiver::decode_current_data;
use crate::fs::GuestPath;
use crate::mem::{ConstPtr, ConstVoidPtr, MutPtr, MutVoidPtr, Ptr};
use crate::objc::{
    autorelease, id, msg, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr,
};
use crate::{msg_class, Environment};

pub(super) struct NSDataHostObject {
    pub(super) bytes: MutVoidPtr,
    pub(super) length: NSUInteger,
    free_when_done: bool,
}
impl HostObject for NSDataHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSData: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSDataHostObject {
        bytes: Ptr::null(),
        length: 0,
        free_when_done: true,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)data {
    let new: id = msg_class![env; NSData alloc];
    let new: id = msg![env; new init];
    autorelease(env, new)
}

+ (id)dataWithBytesNoCopy:(MutVoidPtr)bytes length:(NSUInteger)length freeWhenDone:(bool)free_when_done {
    let new: id = msg_class![env; NSData alloc];
    let new: id = msg![env; new initWithBytesNoCopy:bytes length:length freeWhenDone:free_when_done];
    autorelease(env, new)
}

+ (id)dataWithBytesNoCopy:(MutVoidPtr)bytes length:(NSUInteger)length {
    msg_class![env; NSData dataWithBytesNoCopy:bytes length:length freeWhenDone:true]
}

+ (id)dataWithBytes:(ConstVoidPtr)bytes length:(NSUInteger)length {
    let new: id = msg_class![env; NSData alloc];
    let new: id = msg![env; new initWithBytes:bytes length:length];
    autorelease(env, new)
}

+ (id)dataWithData:(id)data {
    let new: id = msg_class![env; NSData alloc];
    let new: id = msg![env; new initWithData:data];
    autorelease(env, new)
}

+ (id)dataWithContentsOfFile:(id)path {
    let new: id = msg_class![env; NSData alloc];
    let new: id = msg![env; new initWithContentsOfFile:path];
    autorelease(env, new)
}

+ (id)dataWithContentsOfMappedFile:(id)path {
    msg_class![env; NSData dataWithContentsOfFile:path]
}

+ (id)dataWithContentsOfURL:(id)url {
    let new: id = msg_class![env; NSData alloc];
    let new: id = msg![env; new initWithContentsOfURL:url];
    autorelease(env, new)
}

- (id)init {
    let null_ptr: MutVoidPtr = Ptr::null();
    // FIXED: Passed 0u32 so it correctly maps to NSUInteger, preventing the Asphalt 6 Type Mismatch panic!
    msg![env; this initWithBytesNoCopy:null_ptr length:0u32 freeWhenDone:true]
}

- (id)initWithBytesNoCopy:(MutVoidPtr)bytes length:(NSUInteger)length freeWhenDone:(bool)free_when_done {
    let host_object = env.objc.borrow_mut::<NSDataHostObject>(this);
    host_object.bytes = bytes;
    host_object.length = length;
    host_object.free_when_done = free_when_done;
    this
}

- (id)initWithBytesNoCopy:(MutVoidPtr)bytes length:(NSUInteger)length {
    msg![env; this initWithBytesNoCopy:bytes length:length freeWhenDone:true]
}

- (id)initWithBytes:(ConstVoidPtr)bytes length:(NSUInteger)length {
    let new_bytes = env.mem.alloc(length);
    env.mem.memmove(new_bytes, bytes, length);
    msg![env; this initWithBytesNoCopy:new_bytes length:length freeWhenDone:true]
}

- (id)initWithData:(id)data {
    let bytes: ConstVoidPtr = msg![env; data bytes];
    let length: NSUInteger = msg![env; data length];
    msg![env; this initWithBytes:bytes length:length]
}

- (id)initWithContentsOfFile:(id)path {
    let path_str = to_rust_string(env, path);
    let guest_path = GuestPath::new(&path_str);
    if let Some(content) = env.vfs.read(&guest_path) {
        let length = content.len() as u32;
        let bytes = env.mem.alloc_and_write_bytes(&content);
        msg![env; this initWithBytesNoCopy:bytes length:length freeWhenDone:true]
    } else {
        env.objc.dealloc_object(this, &mut env.mem);
        nil
    }
}

- (id)initWithContentsOfURL:(id)url {
    let url_str = to_rust_string(env, msg![env; url absoluteString]);
    if url_str.starts_with("file://") {
        let path: id = msg![env; url path];
        msg![env; this initWithContentsOfFile:path]
    } else {
        // GAMELOFT BYPASS: Forcing nil return on URL fetch to prevent infinite loop!
        log!("GAMELOFT BYPASS: Forcing nil return on URL fetch to prevent infinite loop! URL was: {}", url_str);
        env.objc.dealloc_object(this, &mut env.mem);
        nil
    }
}

- (id)initWithCoder:(id)coder {
    decode_current_data(env, coder, this);
    this
}

- (())dealloc {
    let &NSDataHostObject {
        bytes,
        free_when_done,
        ..
    } = env.objc.borrow(this);
    if free_when_done && !bytes.is_null() {
        let _ = env.mem.free(bytes.cast());
    }
    env.objc.dealloc_object(this, &mut env.mem)
}

- (ConstVoidPtr)bytes {
    env.objc.borrow::<NSDataHostObject>(this).bytes.cast_const()
}

- (NSUInteger)length {
    env.objc.borrow::<NSDataHostObject>(this).length
}

- (id)subdataWithRange:(NSRange)range {
    let &NSDataHostObject { bytes, length, .. } = env.objc.borrow(this);
    assert!(range.location + range.length <= length);
    // FIXED: Syntax error extracted for the macro
    let offset_bytes = bytes + range.location;
    let range_len = range.length;
    msg_class![env; NSData dataWithBytes:offset_bytes length:range_len]
}

- (bool)writeToFile:(id)path atomically:(bool)_atomically {
    let path_str = to_rust_string(env, path);
    let guest_path = GuestPath::new(&path_str);
    
    let &NSDataHostObject { bytes, length, .. } = env.objc.borrow(this);
    let data = env.mem.bytes_at(bytes.cast_const(), length).to_vec();
    
    env.vfs.write(&guest_path, &data).is_ok()
}

@end

@implementation NSMutableData: NSData

+ (id)dataWithCapacity:(NSUInteger)capacity {
    let new: id = msg_class![env; NSMutableData alloc];
    let new: id = msg![env; new initWithCapacity:capacity];
    autorelease(env, new)
}

+ (id)dataWithLength:(NSUInteger)length {
    let new: id = msg_class![env; NSMutableData alloc];
    let new: id = msg![env; new initWithLength:length];
    autorelease(env, new)
}

- (id)initWithCapacity:(NSUInteger)capacity {
    let bytes: MutVoidPtr = if capacity > 0 { env.mem.alloc(capacity) } else { Ptr::null() };
    // FIXED: Passed 0u32 to match NSUInteger TypeId
    msg![env; this initWithBytesNoCopy:bytes length:0u32 freeWhenDone:true]
}

- (id)initWithLength:(NSUInteger)length {
    let bytes: MutVoidPtr = if length > 0 { env.mem.alloc(length) } else { Ptr::null() };
    if length > 0 {
        env.mem.bytes_at_mut(bytes.cast(), length).fill(0);
    }
    msg![env; this initWithBytesNoCopy:bytes length:length freeWhenDone:true]
}

- (())appendData:(id)other_data {
    let other_bytes: ConstVoidPtr = msg![env; other_data bytes];
    let other_bytes: ConstPtr<u8> = other_bytes.cast();
    let other_length: NSUInteger = msg![env; other_data length];
    msg![env; this appendBytes:other_bytes length:other_length]
}

- (())appendBytes:(ConstPtr<u8>)append_bytes length:(NSUInteger)append_length {
    let old_len = env.objc.borrow::<NSDataHostObject>(this).length;
    () = msg![env; this increaseLengthBy:append_length];
    let &NSDataHostObject { bytes, .. } = env.objc.borrow(this);
    env.mem.memmove(bytes + old_len, append_bytes.cast(), append_length);
}

- (MutVoidPtr)mutableBytes {
    let host_obj = env.objc.borrow_mut::<NSDataHostObject>(this);
    host_obj.bytes
}

- (())setLength:(NSUInteger)new_length {
    let &NSDataHostObject {bytes, length, .. } = env.objc.borrow(this);
    let new_bytes = if bytes.is_null() && new_length > 0 {
        env.mem.alloc(new_length)
    } else if new_length > 0 {
        env.mem.realloc(bytes, new_length)
    } else {
        if !bytes.is_null() {
            let _ = env.mem.free(bytes);
        }
        Ptr::null()
    };
    
    if new_length > length && !new_bytes.is_null() {
        env.mem.bytes_at_mut(new_bytes.cast(), new_length)[length as usize..].fill(0);
    }
    let host = env.objc.borrow_mut::<NSDataHostObject>(this);
    host.bytes = new_bytes;
    host.length = new_length;
}

- (())increaseLengthBy:(NSUInteger)extra_length {
    let length = env.objc.borrow::<NSDataHostObject>(this).length;
    // FIXED: Syntax error extracted for the macro
    let new_len = length + extra_length;
    msg![env; this setLength:new_len]
}

- (())replaceBytesInRange:(NSRange)range withBytes:(ConstVoidPtr)bytes {
    let length = env.objc.borrow::<NSDataHostObject>(this).length;
    assert!(range.location + range.length <= length);
    let host_bytes = env.objc.borrow::<NSDataHostObject>(this).bytes;
    env.mem.memmove(host_bytes + range.location, bytes, range.length);
}

@end

};