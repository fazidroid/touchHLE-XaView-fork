/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSURLConnection`.

use crate::objc::{autorelease, id, msg, nil, objc_classes, release, ClassExports};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSURLConnection: NSObject

+ (id)connectionWithRequest:(id)request // NSURLRequest *
                   delegate:(id)delegate {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithRequest:request delegate:delegate];
    autorelease(env, new)
}

- (id)initWithRequest:(id)request // NSURLRequest *
             delegate:(id)delegate {
    msg![env; this initWithRequest:request delegate:delegate startImmediately:true]
}

- (id)initWithRequest:(id)request // NSURLRequest *
             delegate:(id)delegate
     startImmediately:(bool)start_immediately {
    log!(
        "TODO: [(NSURLConnection *){:?} initWithRequest:{:?} delegate:{:?} startImmediately:{}]",
        this,
        request,
        delegate,
        start_immediately,
    );

    // Хак: если игра просит начать загрузку сразу, моментально имитируем ответ
    if start_immediately && delegate != nil {
        // Проверяем, умеет ли игра принимать сигнал об успешной загрузке
        let responds_finish: bool = msg![env; delegate respondsToSelector:crate::sel!(connectionDidFinishLoading:)];
        if responds_finish {
            msg![env; delegate connectionDidFinishLoading:this];
        } else {
            // Если не умеет, шлём сигнал об ошибке сети
            let responds_fail: bool = msg![env; delegate respondsToSelector:crate::sel!(connection:didFailWithError:)];
            if responds_fail {
                msg![env; delegate connection:this didFailWithError:nil];
            }
        }
    }

    // Возвращаем сам объект (this), а не nil, чтобы игра считала подключение рабочим
    this
}

@end

};
