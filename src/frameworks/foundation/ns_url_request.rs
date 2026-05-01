/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSURLRequest and NSMutableURLRequest`.

use super::{ns_string, NSTimeInterval, NSUInteger};
use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::mem::MutPtr;
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::objc::{
    autorelease, id, nil, objc_classes, release, ClassExports, HostObject, NSZonePtr,
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
    http_should_handle_cookies: bool,
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
        http_should_handle_cookies: true,
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

+ (id)sendSynchronousRequest:(id)request returningResponse:(MutPtr<id>)response error:(MutPtr<id>)error {
    log!("sendSynchronousRequest: returning empty data immediately (network disabled)");
    let data = msg_class![env; NSMutableData data];
    if !response.is_null() {
        env.mem.write(response, nil);
    }
    if !error.is_null() {
        let domain = get_static_str(env, "NSURLErrorDomain");
        let code: i32 = -1009; // store the code in a variable
        let err = msg_class![env; NSError errorWithDomain:domain code:code userInfo:nil];
        env.mem.write(error, err);
    }
    autorelease(env, data)
}

// Добавляем базовый init
- (id)initWithURL:(id)url {
    msg![env; this initWithURL:url
                   cachePolicy:NSURLRequestUseProtocolCachePolicy
               timeoutInterval:60.0]
}

- (id)initWithURL:(id)url
        cachePolicy:(NSURLRequestCachePolicy)cache_policy
    timeoutInterval:(NSTimeInterval)timeout_interval {
    // БЕЗОПАСНЫЙ РЕЖИМ
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

- (id)HTTPMethod {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_method
}

- (id)allHTTPHeaderFields {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_header_fields
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

- (())setHTTPShouldHandleCookies:(bool)flag {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).http_should_handle_cookies = flag;
}

// ==========================================================
// 🏎️ EA BYPASS: Absorb the Network Timeout to Prevent Freezes
// ==========================================================
- (())setTimeoutInterval:(NSTimeInterval)timeout_interval {
    // Safely store the timeout value so the C++ engine is satisfied
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).timeout_interval = timeout_interval;
    println!("🎮 LOG: NSMutableURLRequest setTimeoutInterval: {} safely absorbed!", timeout_interval);
}

- (())addValue:(id)value // NSString*
    forHTTPHeaderField:(id)field { // NSString*
    if value == nil || field == nil { return; }
    log_dbg!("[(NSMutableURLRequest*){:?} addValue:'{}' forHTTPHeaderField:'{}']", this, to_rust_string(env, value), to_rust_string(env, field));

    let host_obj = env.objc.borrow_mut::<NSURLRequestHostObject>(this);
    let http_header_fields = host_obj.http_header_fields;

    // Check if a value already exists for this field
    let existing_value: id = msg![env; http_header_fields objectForKey:field];
    if existing_value != nil {
        // Append the new value with a comma separator
        let separator = ns_string::get_static_str(env, ", ");
        let combined: id = msg![env; existing_value stringByAppendingString:separator];
        let combined: id = msg![env; combined stringByAppendingString:value];
        () = msg![env; http_header_fields setObject:combined forKey:field];
    } else {
        // No existing value, just set it
        () = msg![env; http_header_fields setObject:value forKey:field];
    }
}

- (())setAllHTTPHeaderFields:(id)headers { // NSDictionary *
    if headers == nil { return; }
    let headers_copy = msg![env; headers copy];
    let host_obj = env.objc.borrow_mut::<NSURLRequestHostObject>(this);
    let old_headers = std::mem::replace(&mut host_obj.http_header_fields, headers_copy);
    release(env, old_headers);
}

- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    let new: id = msg_class![env; NSMutableURLRequest alloc];
    let url: id = msg![env; this URL];
    let new: id = msg![env; new initWithURL:url];
    let method: id = msg![env; this HTTPMethod];
    if method != nil {
        () = msg![env; new setHTTPMethod:method];
    }
    let headers: id = msg![env; this allHTTPHeaderFields];
    if headers != nil {
        () = msg![env; new setAllHTTPHeaderFields:headers];
    }
    let body: id = msg![env; this HTTPBody];
    if body != nil {
        () = msg![env; new setHTTPBody:body];
    }
    new
}

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
