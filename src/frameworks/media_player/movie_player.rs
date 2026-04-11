/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMoviePlayerController` etc.

use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::foundation::{ns_string, ns_url, NSInteger};
use crate::frameworks::uikit::ui_device::UIDeviceOrientation;
use crate::objc::{
    id, msg, msg_class, nil, objc_classes, release, retain, todo_objc_setter, ClassExports,
    HostObject, NSZonePtr,
};
use crate::Environment;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct State {
    active_player: Option<id>,
    pending_notifications: VecDeque<(&'static str, id, Instant)>,
}
impl State {
    fn get(env: &mut Environment) -> &mut Self {
        &mut env.framework_state.media_player.movie_player
    }
}

type MPMovieScalingMode = NSInteger;
type MPMovieControlStyle = NSInteger;

type MPMoviePlaybackState = NSInteger;
const MPMoviePlaybackStateStopped: MPMoviePlaybackState = 0;

pub const MPMoviePlayerPlaybackDidFinishNotification: &str =
    "MPMoviePlayerPlaybackDidFinishNotification";
pub const MPMoviePlayerContentPreloadDidFinishNotification: &str =
    "MPMoviePlayerContentPreloadDidFinishNotification";
pub const MPMoviePlayerScalingModeDidChangeNotification: &str =
    "MPMoviePlayerScalingModeDidChangeNotification";
const MPMoviePlayerPlaybackDidFinishReasonUserInfoKey: &str =
    "MPMoviePlayerPlaybackDidFinishReasonUserInfoKey";

pub const CONSTANTS: ConstantExports = &[
    (
        "_MPMoviePlayerPlaybackDidFinishNotification",
        HostConstant::NSString(MPMoviePlayerPlaybackDidFinishNotification),
    ),
    (
        "_MPMoviePlayerContentPreloadDidFinishNotification",
        HostConstant::NSString(MPMoviePlayerContentPreloadDidFinishNotification),
    ),
    (
        "_MPMoviePlayerScalingModeDidChangeNotification",
        HostConstant::NSString(MPMoviePlayerScalingModeDidChangeNotification),
    ),
    (
        "_MPMoviePlayerPlaybackDidFinishReasonUserInfoKey",
        HostConstant::NSString(MPMoviePlayerPlaybackDidFinishReasonUserInfoKey),
    ),
];

struct MPMoviePlayerControllerHostObject {
    content_url: id,
}
impl HostObject for MPMoviePlayerControllerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMoviePlayerController: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(MPMoviePlayerControllerHostObject {
        content_url: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithContentURL:(id)url { // NSURL*
    log!(
        "TODO: [(MPMoviePlayerController*){:?} initWithContentURL:{:?} ({:?})]",
        this,
        url,
        ns_url::to_rust_path(env, url),
    );

    retain(env, url);
    env.objc.borrow_mut::<MPMoviePlayerControllerHostObject>(this).content_url = url;

    // 🏎️ GAMELOFT BYPASS: Fix the Race Condition!
    // We delay the finish notification by 500ms. If we fire it instantly,
    // the game misses the event because it hasn't executed `addObserver` yet!
    State::get(env).pending_notifications.push_back(
        (MPMoviePlayerContentPreloadDidFinishNotification, this, Instant::now() + Duration::from_millis(200))
    );

    State::get(env).pending_notifications.push_back(
        (MPMoviePlayerPlaybackDidFinishNotification, this, Instant::now() + Duration::from_millis(500))
    );

    this
}

- (())dealloc {
    let url = env.objc.borrow::<MPMoviePlayerControllerHostObject>(this).content_url;
    release(env, url);

    env.objc.dealloc_object(this, &mut env.mem);
}

- (id)contentURL {
    env.objc.borrow::<MPMoviePlayerControllerHostObject>(this).content_url
}

- (id)backgroundColor {
    msg_class![env; UIColor blackColor] 
}
- (())setBackgroundColor:(id)color { 
    todo_objc_setter!(this, color);
}

- (())setScalingMode:(MPMovieScalingMode)mode {
    todo_objc_setter!(this, mode);
}
- (())setUseApplicationAudioSession:(bool)use_session {
    todo_objc_setter!(this, use_session);
}
- (())setControlStyle:(MPMovieControlStyle)style {
    todo_objc_setter!(this, style);
}
- (())setFullscreen:(bool)fullsreen {
    todo_objc_setter!(this, fullsreen);
}

- (id)view {
    nil 
}

- (MPMoviePlaybackState)playbackState {
    MPMoviePlaybackStateStopped 
}

- (())setMovieControlMode:(NSInteger)_mode {}

- (())setOrientation:(UIDeviceOrientation)_orientation animated:(bool)_animated {}

- (())play {
    log!("TODO: [(MPMoviePlayerController*){:?} play]", this);
    if let Some(old) = env.framework_state.media_player.movie_player.active_player {
        let _: () = msg![env; old stop];
    }
    assert!(env.framework_state.media_player.movie_player.active_player.is_none());
    retain(env, this);
    env.framework_state.media_player.movie_player.active_player = Some(this);

    // 🏎️ GAMELOFT BYPASS: Fire completion on play with a tiny delay
    let notif = (MPMoviePlayerPlaybackDidFinishNotification, this, Instant::now() + Duration::from_millis(100));
    for (name, obj, _) in &mut State::get(env).pending_notifications {
        if *name == MPMoviePlayerPlaybackDidFinishNotification && *obj == this {
            return; 
        }
    }
    State::get(env).pending_notifications.push_back(notif);
}

- (())pause {
    log!("TODO: [(MPMoviePlayerController*){:?} pause]", this);
}

- (())stop {
    log!("TODO: [(MPMoviePlayerController*){:?} stop]", this);
    if env.framework_state.media_player.movie_player.active_player.is_some() {
        assert!(this == env.framework_state.media_player.movie_player.active_player.take().unwrap());
        release(env, this);
    }
}

@end

@implementation MPMoviePlayerViewController: UIViewController

- (id)initWithContentURL:(id)url {
    log!("🏎️ GAMELOFT BYPASS: [(MPMoviePlayerViewController*){:?} initWithContentURL:{:?}]", this, url);
    // 🏎️ FIX: DO NOT return nil here! The game needs a valid ViewController!
    
    // Queue the fake finish event so the game moves on
    State::get(env).pending_notifications.push_back(
        (MPMoviePlayerPlaybackDidFinishNotification, this, Instant::now() + Duration::from_millis(500))
    );
    
    this
}

- (id)moviePlayer {
    // Return nil so the game safely ignores calls to [vc.moviePlayer play] without crashing
    nil
}

@end

};

pub(super) fn handle_players(env: &mut Environment) {
    let mut notifs_to_run = Vec::new();
    let pending_notifs = &mut State::get(env).pending_notifications;
    let mut i = 0;
    while i < pending_notifs.len() {
        let (name_str, object, time) = pending_notifs[i];
        if Instant::now() >= time {
            notifs_to_run.push((name_str, object));
            pending_notifs.swap_remove_back(i);
        } else {
            i += 1;
        }
    }
    for (name_str, object) in notifs_to_run {
        let name = ns_string::get_static_str(env, name_str);
        let center: id = msg_class![env; NSNotificationCenter defaultCenter];
        let _: () = msg![env; center postNotificationName:name object:object];
    }
}
