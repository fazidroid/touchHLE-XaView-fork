/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSURLRequest and NSMutableURLRequest`.

use super::{ns_string, NSTimeInterval, NSUInteger};
use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::objc::{
    autorelease, id, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr,
};
use crate::{msg, msg_class};

type NSURLRequestCachePolicy = NSUInteger;
const NSURLRequestUseProtocolCachePolicy: NSURLRequestCachePolicy = 0;

struct NSURLRequestHostObject {
    /// `NSURL*`
    url: id,
    cache_policy: NSURLRequestCachePolicy,
    timeout_interval: NSTimeInterval,
    // Request components
    /// `NSString*`
    http_method: id,
    /// `NSData*`
    http_body: id,
    // Header fields
    /// `NSDictionary*`
    http_header_fields: id,
}
impl HostObject for NSURLRequestHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSURLRequest: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let http_header_fields: id = msg_class![env; NSMutableDictionary new];
    let host_object = Box::new(NSURLRequestHostObject {
        url: nil,
        cache_policy: NSURLRequestUseProtocolCachePolicy,
        timeout_interval: 60.0,
        http_method: ns_string::get_static_str(env, "GET"),
        http_body: nil,
        http_header_fields,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)requestWithURL:(id)url {
    msg![env; this requestWithURL:url
                      cachePolicy:NSURLRequestUseProtocolCachePolicy
                  timeoutInterval:60.0]
}

+ (id)requestWithURL:(id)url
         cachePolicy:(NSURLRequestCachePolicy)cache_policy
     timeoutInterval:(NSTimeInterval)timeout_interval {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithURL:url
                                cachePolicy:cache_policy
                            timeoutInterval:timeout_interval];
    autorelease(env, new)
}

// Добавляем базовый init, который так нужен играм Gameloft
- (id)initWithURL:(id)url {
    msg![env; this initWithURL:url
                   cachePolicy:NSURLRequestUseProtocolCachePolicy
               timeoutInterval:60.0]
}

- (id)initWithURL:(id)url
        cachePolicy:(NSURLRequestCachePolicy)cache_policy
    timeoutInterval:(NSTimeInterval)timeout_interval {
    
    // БЕЗОПАСНЫЙ РЕЖИМ: мы больше никогда не возвращаем nil и не делаем release(env, this), 
    // чтобы C++ обертки игры не вызывали Use-After-Free краши.
    
    if url != nil {
        let url_copy = msg![env; url copy];
        env.objc.borrow_mut::<NSURLRequestHostObject>(this).url = url_copy;
    }
    
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).cache_policy = cache_policy;
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).timeout_interval = timeout_interval;

    if !env.options.network_access {
        log!("Network access is disabled, but returning valid NSURLRequest to prevent C++ crashes");
    }

    this
}

- (id)URL {
    env.objc.borrow::<NSURLRequestHostObject>(this).url
}
- (id)HTTPBody {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_body
}

- (())dealloc {
    log_dbg!("[(NSURLRequest*){:?} dealloc]", this);
    let &NSURLRequestHostObject {
        url,
        http_method,
        http_body,
        http_header_fields,
        ..
    } = env.objc.borrow(this);
    release(env, url);
    release(env, http_method);
    release(env, http_body);
    release(env, http_header_fields);
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

@implementation NSMutableURLRequest: NSURLRequest

- (())setHTTPMethod:(id)http_method { // NSString *
    if http_method == nil { return; }
    let http_method_copy = msg![env; http_method copy];
    let host_obj = env.objc.borrow_mut::<NSURLRequestHostObject>(this);
    let old_http_method = std::mem::replace(&mut host_obj.http_method, http_method_copy);
    release(env, old_http_method);
}

- (())setHTTPBody:(id)http_body { // NSData *
    if http_body == nil { return; }
    let http_body_copy = msg![env; http_body copy];
    let host_obj = env.objc.borrow_mut::<NSURLRequestHostObject>(this);
    let old_http_body = std::mem::replace(&mut host_obj.http_body, http_body_copy);
    release(env, old_http_body);
}

- (())setValue:(id)value // NSString *
    forHTTPHeaderField:(id)field { // NSString *
    if value == nil || field == nil { return; }
    log_dbg!("[(NSURLRequest*){:?} setValue:'{}' forHTTPHeaderField:'{}']", this, to_rust_string(env, value), to_rust_string(env, field));
    let http_header_fields = env.objc.borrow_mut::<NSURLRequestHostObject>(this).http_header_fields;
    () = msg![env; http_header_fields setObject:value forKey:field];
}

@end

};