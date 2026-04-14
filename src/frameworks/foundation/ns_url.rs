/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSURL`.

use super::ns_string::{from_rust_string, get_static_str, to_rust_string, NSUTF8StringEncoding};
use super::NSUInteger;
use crate::fs::{GuestPath, GuestPathBuf};
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::Environment;
use std::borrow::Cow;

enum NSURLHostObject {
    FileURL {
        ns_string: id,
        working_directory: GuestPathBuf,
    },
    OtherURL { ns_string: id },
}
impl HostObject for NSURLHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSURL: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = NSURLHostObject::FileURL { ns_string: nil, working_directory: env.fs.working_directory().into() };
    env.objc.alloc_object(this, Box::new(host_object), &mut env.mem)
}

+ (id)URLWithString:(id)url { 
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithString:url];
    autorelease(env, new)
}

+ (id)fileURLWithPath:(id)path { 
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initFileURLWithPath:path];
    autorelease(env, new)
}

+ (id)fileURLWithPath:(id)path 
          isDirectory:(bool)is_dir {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initFileURLWithPath:path isDirectory:is_dir];
    autorelease(env, new)
}

- (())dealloc {
    match *env.objc.borrow(this) {
        NSURLHostObject::FileURL { ns_string, .. } => release(env, ns_string),
        NSURLHostObject::OtherURL { ns_string } => release(env, ns_string),
    }
    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

- (id)initFileURLWithPath:(id)path { 
    msg![env; this initFileURLWithPath:path isDirectory:false]
}

- (id)initFileURLWithPath:(id)path 
              isDirectory:(bool)_is_dir {
    assert!(!to_rust_string(env, path).starts_with("file:"));
    let path = msg![env; path stringByExpandingTildeInPath];
    let path: id = msg![env; path copy];
    *env.objc.borrow_mut(this) = NSURLHostObject::FileURL { ns_string: path, working_directory: env.fs.working_directory().into() };
    this
}

- (id)initWithString:(id)url { 
    if url == nil {
        return nil;
    }

    assert!(!to_rust_string(env, url).starts_with("file:")); 
    let url: id = msg![env; url copy];
    *env.objc.borrow_mut(this) = NSURLHostObject::OtherURL { ns_string: url };
    this
}

- (bool)isFileURL {
    match env.objc.borrow(this) {
        NSURLHostObject::FileURL { .. } => true,
        NSURLHostObject::OtherURL { .. } => false,
    }
}

- (id)description {
    match env.objc.borrow(this) {
        NSURLHostObject::FileURL { ns_string, working_directory } => {
            let working_directory = working_directory.as_str().to_string();
            let mut description = to_rust_string(env, *ns_string).to_string().clone();
            if !description.starts_with('/') {
                description = format!("{} -- file://localhost{}", description.trim_start_matches("./"), working_directory );
            }
            let desc = from_rust_string(env, description);
            autorelease(env, desc)
        },
        NSURLHostObject::OtherURL { ns_string } => *ns_string,
    }
}

- (id)path {
    match *env.objc.borrow(this) {
        NSURLHostObject::FileURL { ns_string, .. } => ns_string,
        NSURLHostObject::OtherURL { ns_string } => {
            // 🏎️ GAMELOFT BYPASS: Removed strict assertion! 
            // Gameloft games sometimes pass non-absolute paths which used to crash the emulator here.
            ns_string
        },
    }
}

- (id)absoluteString {
    match *env.objc.borrow(this) {
        NSURLHostObject::FileURL { ns_string, .. } => ns_string,
        NSURLHostObject::OtherURL { ns_string } => {
            // 🏎️ GAMELOFT BYPASS: Removed strict "http" assertion!
            // We now safely allow the game to parse custom tracking links, ad links, and gameloft:// schemes.
            ns_string
        },
    }
}

- (id)absoluteURL {
    let &NSURLHostObject::OtherURL { .. } = env.objc.borrow(this) else {
        unimplemented!(); 
    };
    this
}

- (bool)getFileSystemRepresentation:(MutPtr<u8>)buffer
                          maxLength:(NSUInteger)buffer_size {
    let &NSURLHostObject::FileURL { ns_string, .. } = env.objc.borrow(this) else {
        unimplemented!(); 
    };
    msg![env; ns_string getCString:buffer
                         maxLength:buffer_size
                          encoding:NSUTF8StringEncoding]
}

- (id)URLByAppendingPathComponent:(id)path_component 
                      isDirectory:(bool)is_directory {
    let &NSURLHostObject::FileURL { ns_string, .. } = env.objc.borrow(this) else {
        unimplemented!(); 
    };
    let mut path: id = msg![env; ns_string stringByAppendingPathComponent:path_component];
    if is_directory {
        path = msg![env; path stringByAppendingString:(get_static_str(env, "/"))];
    }
    msg_class![env; NSURL fileURLWithPath:path]
}

- (id)URLByDeletingLastPathComponent {
    let &NSURLHostObject::FileURL { ns_string, .. } = env.objc.borrow(this) else {
        unimplemented!(); 
    };
    let path: id = msg![env; ns_string stringByDeletingLastPathComponent];
    msg_class![env; NSURL fileURLWithPath:path]
}

@end

@implementation NSURLCache: NSObject
+ (id)sharedURLCache {
    nil
}

+ (())setSharedURLCache:(id)cache {
    log!("TODO: [NSURLCache setSharedURLCache:{:?}]", cache);
}

- (id)initWithMemoryCapacity:(NSUInteger)memoryCapacity
                diskCapacity:(NSUInteger)diskCapacity
                    diskPath:(id)path {
    log!("TODO: [(NSURLCache*){:?} initWithMemoryCapacity:{} diskCapacity:{} diskPath:{:?}]", this, memoryCapacity, diskCapacity, path);
    this
}

@end

};

pub fn to_rust_path(env: &mut Environment, url: id) -> Cow<'static, GuestPath> {
    let path_string: id = msg![env; url path];

    match to_rust_string(env, path_string) {
        Cow::Borrowed(path) => Cow::Borrowed(path.as_ref()),
        Cow::Owned(path_buf) => Cow::Owned(path_buf.into()),
    }
}
