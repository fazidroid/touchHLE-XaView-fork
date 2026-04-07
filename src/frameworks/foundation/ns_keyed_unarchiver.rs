/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSKeyedUnarchiver` and deserialization of its object graph format.

use super::ns_string::{from_rust_string, get_static_str, to_rust_string};
use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::foundation::{NSInteger, NSUInteger};
use crate::mem::{ConstPtr, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::Environment;
use plist::{Dictionary, Uid, Value};
use std::io::Cursor;

pub const NSKeyedArchiveRootObjectKey: &str = "root";

pub const CONSTANTS: ConstantExports = &[(
    "_NSKeyedArchiveRootObjectKey",
    HostConstant::NSString(NSKeyedArchiveRootObjectKey),
)];

struct NSKeyedUnarchiverHostObject {
    plist: Dictionary,
    current_key: Option<Uid>,
    already_unarchived: Vec<Option<id>>,
    delegate: id,
    temporary_buffers: Vec<MutVoidPtr>,
}
impl HostObject for NSKeyedUnarchiverHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSKeyedUnarchiver: NSCoder

+ (id)allocWithZone:(NSZonePtr)_zone {
    let unarchiver = Box::new(NSKeyedUnarchiverHostObject {
        plist: Dictionary::new(),
        current_key: None,
        already_unarchived: Vec::new(),
        delegate: nil,
        temporary_buffers: Vec::new(),
    });
    env.objc.alloc_object(this, unarchiver, &mut env.mem)
}

+ (id)unarchiveObjectWithFile:(id)path {
    let data: id = msg_class![env; NSData dataWithContentsOfFile:path];
    if (data == nil) { return nil; }
    msg![env; this unarchiveObjectWithData:data]
}

+ (id)unarchiveObjectWithData:(id)data {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initForReadingWithData:data];
    let root_key = get_static_str(env, NSKeyedArchiveRootObjectKey);
    let result: id = msg![env; new decodeObjectForKey:root_key];
    autorelease(env, result)
}

- (id)initForReadingWithData:(id)data {
    if (data == nil) { return nil; }

    let length: NSUInteger = msg![env; data length];
    let bytes: ConstVoidPtr = msg![env; data bytes];
    let slice = env.mem.bytes_at(bytes.cast(), length);

    let host_obj = env.objc.borrow_mut::<NSKeyedUnarchiverHostObject>(this);
    if let Ok(plist_val) = Value::from_reader(Cursor::new(slice)) {
        if let Some(dict) = plist_val.into_dictionary() {
            let key_count = dict.get("$objects").and_then(|o| o.as_array()).map(|a| a.len()).unwrap_or(0);
            host_obj.already_unarchived = vec![None; key_count];
            host_obj.plist = dict;
        }
    }
    this
}

- (())dealloc {
    let host_obj = borrow_host_obj(env, this);
    let already_unarchived = std::mem::take(&mut host_obj.already_unarchived);
    let temporary_buffers = std::mem::take(&mut host_obj.temporary_buffers);
    for &object in already_unarchived.iter().flatten() {
        release(env, object);
    }
    for &buffer in temporary_buffers.iter() {
        env.mem.free(buffer);
    }
    env.objc.dealloc_object(this, &mut env.mem)
}

- (bool)decodeBoolForKey:(id)key {
    get_value_to_decode_for_key(env, this, key)
        .map_or(false, |value| value.as_boolean().unwrap_or(false))
}

- (f64)decodeDoubleForKey:(id)key {
    get_value_to_decode_for_key(env, this, key).map_or(0.0, |value| value.as_real().unwrap_or(0.0))
}

- (f32)decodeFloatForKey:(id)key {
    get_value_to_decode_for_key(env, this, key).map_or(0.0, |value| value.as_real().unwrap_or(0.0)) as f32
}

- (NSInteger)decodeIntegerForKey:(id)key {
    get_value_to_decode_for_key(env, this, key).map_or(0, |value| value.as_signed_integer().unwrap_or(0)) as NSInteger
}

- (i32)decodeIntForKey:(id)key {
    get_value_to_decode_for_key(env, this, key).map_or(0, |value| value.as_signed_integer().unwrap_or(0)) as i32
}

- (id)decodeObjectForKey:(id)key {
    let val_opt = get_value_to_decode_for_key(env, this, key).and_then(|v| v.as_uid().copied());
    if let Some(next_uid) = val_opt {
        let object = unarchive_key(env, this, next_uid);
        retain(env, object);
        return autorelease(env, object);
    }
    nil
}

@end

};

fn borrow_host_obj(env: &mut Environment, unarchiver: id) -> &mut NSKeyedUnarchiverHostObject {
    env.objc.borrow_mut(unarchiver)
}

fn get_value_to_decode_for_key(env: &mut Environment, unarchiver: id, key: id) -> Option<&Value> {
    let key_str = to_rust_string(env, key);
    let host_obj = env.objc.borrow_mut::<NSKeyedUnarchiverHostObject>(unarchiver);
    let scope = match host_obj.current_key {
        Some(uid) => &host_obj.plist["$objects"].as_array().unwrap()[uid.get() as usize],
        None => &host_obj.plist["$top"],
    }.as_dictionary()?;
    scope.get(&*key_str)
}

fn unarchive_key(env: &mut Environment, unarchiver: id, key: Uid) -> id {
    if let Some(existing) = borrow_host_obj(env, unarchiver).already_unarchived[key.get() as usize] {
        return existing;
    }

    let item = borrow_host_obj(env, unarchiver).plist["$objects"].as_array().unwrap()[key.get() as usize].clone();

    let new_object = match item {
        Value::Dictionary(dict) => {
            let class_key = dict["$class"].as_uid().copied().unwrap();
            let class = if let Some(existing) = borrow_host_obj(env, unarchiver).already_unarchived[class_key.get() as usize] {
                existing
            } else {
                let class_name = borrow_host_obj(env, unarchiver).plist["$objects"].as_array().unwrap()[class_key.get() as usize]
                    .as_dictionary().unwrap()["$classname"].as_string().unwrap().to_string();
                let cls = env.objc.get_known_class(&class_name, &mut env.mem);
                borrow_host_obj(env, unarchiver).already_unarchived[class_key.get() as usize] = Some(cls);
                cls
            };

            let old_key = borrow_host_obj(env, unarchiver).current_key;
            borrow_host_obj(env, unarchiver).current_key = Some(key);
            let new_obj: id = msg![env; class alloc];
            let new_obj: id = msg![env; new_obj initWithCoder:unarchiver];
            borrow_host_obj(env, unarchiver).current_key = old_key;
            new_obj
        }
        Value::String(s) => from_rust_string(env, s.to_string()),
        Value::Integer(int) => {
            let num: id = msg_class![env; NSNumber alloc];
            if let Some(i) = int.as_signed() { msg![env; num initWithLongLong:i] }
            else { 
                let u = int.as_unsigned().unwrap_or(0);
                msg![env; num initWithUnsignedLongLong:u] 
            }
        }
        _ => nil,
    };
    borrow_host_obj(env, unarchiver).already_unarchived[key.get() as usize] = Some(new_object);
    new_object
}

// FIXED: Added missing functions required by NSArray and NSDictionary
pub fn decode_current_array(env: &mut Environment, unarchiver: id) -> Vec<id> {
    let keys = keys_for_key(env, unarchiver, "NS.objects");
    keys.into_iter().map(|k| retain(env, unarchive_key(env, unarchiver, k))).collect()
}

pub fn decode_current_dict(env: &mut Environment, unarchiver: id) -> Vec<(id, id)> {
    let ks = keys_for_key(env, unarchiver, "NS.keys");
    let vs = keys_for_key(env, unarchiver, "NS.objects");
    ks.into_iter().zip(vs).map(|(k, v)| (unarchive_key(env, unarchiver, k), unarchive_key(env, unarchiver, v))).collect()
}

pub fn decode_current_date(env: &mut Environment, unarchiver: id) -> id {
    let k = get_static_str(env, "NS.time");
    let val_opt = get_value_to_decode_for_key(env, unarchiver, k).and_then(|v| v.as_real());
    if let Some(val) = val_opt {
        let date: id = msg_class![env; NSDate alloc];
        return msg![env; date initWithTimeIntervalSinceReferenceDate:val];
    }
    nil
}

// FIXED: Resolved double mutable borrow of env.mem
pub fn decode_current_data(env: &mut Environment, unarchiver: id, is_mutable: bool) -> id {
    let k = get_static_str(env, "NS.data");
    let bytes_vec = get_value_to_decode_for_key(env, unarchiver, k).and_then(|v| v.as_data().map(|d| d.to_vec()));
    if let Some(bytes) = bytes_vec {
        let len: GuestUSize = bytes.len() as GuestUSize;
        let g_bytes = env.mem.alloc(len);
        env.mem.bytes_at_mut(g_bytes.cast(), len).copy_from_slice(&bytes);
        let cls_name = if is_mutable { "NSMutableData" } else { "NSData" };
        let cls = env.objc.get_known_class(cls_name, &mut env.mem);
        let data: id = msg![env; cls alloc];
        return msg![env; data initWithBytesNoCopy:g_bytes length:len freeWhenDone:true];
    }
    nil
}

fn keys_for_key(env: &mut Environment, unarchiver: id, key: &str) -> Vec<Uid> {
    let host_obj = borrow_host_obj(env, unarchiver);
    if let Some(curr) = host_obj.current_key {
        let objects = &host_obj.plist["$objects"];
        if let Some(dict) = objects.as_array().unwrap()[curr.get() as usize].as_dictionary() {
            if let Some(Value::Array(keys)) = dict.get(key) {
                return keys.iter().filter_map(|v| v.as_uid().copied()).collect();
            }
        }
    }
    Vec::new()
}
