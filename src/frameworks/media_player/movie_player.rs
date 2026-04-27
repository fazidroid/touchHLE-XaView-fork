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
    /// Various apps (e.g. Crash Bandicoot Nitro Kart 3D and Spore Origins)
    /// create or start a player and await some kind of notification, but can't
    /// handle it if that notification happens immediately. This queue lets us
    /// delay such notifications until the app next returns to the run loop,
    /// which seems to be late enough.
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

// Values might not be correct, but as these are linked symbol constants, it
// shouldn't matter.
pub const MPMovieDurationAvailableNotification: &str =
    "MPMovieDurationAvailableNotification";

pub const MPMoviePlayerPlaybackDidFinishNotification: &str =
    "MPMoviePlayerPlaybackDidFinishNotification";
/// Apparently an undocumented, private API. Spore Origins uses it.
pub const MPMoviePlayerContentPreloadDidFinishNotification: &str =
    "MPMoviePlayerContentPreloadDidFinishNotification";
pub const MPMoviePlayerScalingModeDidChangeNotification: &str =
    "MPMoviePlayerScalingModeDidChangeNotification";
pub const MPMoviePlayerLoadStateDidChangeNotification: &str =
    "MPMoviePlayerLoadStateDidChangeNotification";
pub const MPMoviePlayerPlaybackStateDidChangeNotification: &str =
    "MPMoviePlayerPlaybackStateDidChangeNotification";
pub const MPMoviePlayerWillExitFullscreenNotification: &str =
    "MPMoviePlayerWillExitFullscreenNotification";
pub const MPMoviePlayerDidExitFullscreenNotification: &str =
    "MPMoviePlayerDidExitFullscreenNotification";
pub const MPMoviePlayerWillEnterFullscreenNotification: &str =
    "MPMoviePlayerWillEnterFullscreenNotification";
pub const MPMoviePlayerDidEnterFullscreenNotification: &str =
    "MPMoviePlayerDidEnterFullscreenNotification";
// TODO: More notifications?
const MPMoviePlayerPlaybackDidFinishReasonUserInfoKey: &str =
    "MPMoviePlayerPlaybackDidFinishReasonUserInfoKey";

/// `NSNotificationName` values and other constants.
pub const CONSTANTS: ConstantExports = &[
    (
        "_MPMovieDurationAvailableNotification",
        HostConstant::NSString(MPMovieDurationAvailableNotification),
    ),
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
        "_MPMoviePlayerLoadStateDidChangeNotification",
        HostConstant::NSString(MPMoviePlayerLoadStateDidChangeNotification),
    ),
    (
        "_MPMoviePlayerPlaybackStateDidChangeNotification",
        HostConstant::NSString(MPMoviePlayerPlaybackStateDidChangeNotification),
    ),
    (
        "_MPMoviePlayerWillExitFullscreenNotification",
        HostConstant::NSString(MPMoviePlayerWillExitFullscreenNotification),
    ),
    (
        "_MPMoviePlayerDidExitFullscreenNotification",
        HostConstant::NSString(MPMoviePlayerDidExitFullscreenNotification),
    ),
    (
        "_MPMoviePlayerWillEnterFullscreenNotification",
        HostConstant::NSString(MPMoviePlayerWillEnterFullscreenNotification),
    ),
    (
        "_MPMoviePlayerDidEnterFullscreenNotification",
        HostConstant::NSString(MPMoviePlayerDidEnterFullscreenNotification),
    ),
    (
        "_MPMoviePlayerPlaybackDidFinishReasonUserInfoKey",
        HostConstant::NSString(MPMoviePlayerPlaybackDidFinishReasonUserInfoKey),
    ),
];

struct MPMoviePlayerControllerHostObject {
    // NSURL *
    content_url: id,
}
impl HostObject for MPMoviePlayerControllerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMoviePlayerController: NSObject

// TODO: actual playback

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

    // Act as if loading immediately completed (Spore Origins waits for this).
    State::get(env).pending_notifications.push_back(
        (MPMoviePlayerContentPreloadDidFinishNotification, this, Instant::now())
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
    msg_class![env; UIColor blackColor] // TODO
}
- (())setBackgroundColor:(id)color { // UIColor*
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
    nil // TODO
}

- (MPMoviePlaybackState)playbackState {
    MPMoviePlaybackStateStopped // TODO
}

// Apparently an undocumented, private API, but Spore Origins uses it.
- (())setMovieControlMode:(NSInteger)_mode {
    // As this is undocumented and we don't have real video playback yet, let's
    // ignore it.
}

// Another undocumented one! But some apps may still use it :/
// https://stackoverflow.com/a/1390079/2241008
- (())setOrientation:(UIDeviceOrientation)_orientation animated:(bool)_animated {

}

// MPMediaPlayback implementation
- (())play {
        // ==========================================================
        // 🏎️ DYNAMIC VIDEO BYPASS: EA-Exclusive Instant Completion
        // ==========================================================
        let mut is_ea_game = false;
        
        // Safety check to prevent crashing the app picker!
        if !env.is_app_picker {
            let bundle_id = env.bundle.bundle_identifier();
            is_ea_game = bundle_id.starts_with("com.ea")
                || bundle_id.starts_with("com.firemint")
                || bundle_id.starts_with("com.gameloft")
                || bundle_id.starts_with("com.namco");
        }

        if is_ea_game {
            log_dbg!("MPMoviePlayerController play: faking instant video completion");
            
            let center: id = msg_class![env; NSNotificationCenter defaultCenter];
            
            // 1. Tell the game the video duration is available
            let duration_notif = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMovieDurationAvailableNotification".to_string());
            let _: () = msg![env; center postNotificationName:duration_notif object:this];

            // 2. Broadcast that the video playback has completely finished!
            let finish_notif = crate::frameworks::foundation::ns_string::from_rust_string(env, "MPMoviePlayerPlaybackDidFinishNotification".to_string());
            let _: () = msg![env; center postNotificationName:finish_notif object:this];
        } else {
            // Standard touchHLE behavior for Gameloft and other developers
            println!("🎮 LOG: MPMoviePlayerController play called. Leaving standard behavior intact.");
            // (If the original movie_player.rs implements actual video playback in the future, it goes here)
        }
    }

- (())pause {
    log!("TODO: [(MPMoviePlayerController*){:?} pause]", this);
}

- (())stop {
    log!("TODO: [(MPMoviePlayerController*){:?} stop]", this);
    if env.framework_state.media_player.movie_player.active_player.is_some() {
        // Some applications (like NOVA2) may send 2 `stop` messages for each
        // 1 `play` message for the player. In that case, we want to release
        // the active player only once.
        assert!(this == env.framework_state.media_player.movie_player.active_player.take().unwrap());
        release(env, this);
    }
}

@end

@implementation MPMoviePlayerViewController: UIViewController

    - (id)initWithContentURL:(id)url {
        log_dbg!("MPMoviePlayerViewController initWithContentURL: faking instant completion");

        // Initialise the view controller normally so it has a valid object.
        let this: id = crate::msg_super![env; this init];
        if this == nil {
            return nil;
        }

        // Fire notifications on BOTH the view controller AND a fake inner
        // MPMoviePlayerController. Asphalt 6 (and similar Gameloft games)
        // use a custom subclass (LandscapeMoviePlayerViewController) and
        // register notification observers on the *inner player* object, not
        // on the view controller itself.
        let center: id = msg_class![env; NSNotificationCenter defaultCenter];

        // Create a fake inner MPMoviePlayerController so observers that watch
        // it receive the completion notifications.
        let inner: id = msg_class![env; MPMoviePlayerController alloc];
        let inner: id = msg![env; inner init];

        // Fire on both the VC (this) and the inner player.
        for &sender in &[this, inner] {
            // 1. Load state changed — video is "ready to play".
            let n = ns_string::get_static_str(env, MPMoviePlayerLoadStateDidChangeNotification);
            let _: () = msg![env; center postNotificationName:n object:sender];

            // 2. Duration is known.
            let n2 = crate::frameworks::foundation::ns_string::from_rust_string(
                env, "MPMovieDurationAvailableNotification".to_string());
            let _: () = msg![env; center postNotificationName:n2 object:sender];

            // 3. Playback state changed to playing, then stopped.
            let n3 = ns_string::get_static_str(env, MPMoviePlayerPlaybackStateDidChangeNotification);
            let _: () = msg![env; center postNotificationName:n3 object:sender];

            // 4. Playback finished — game advances past intro/video.
            let n4 = ns_string::get_static_str(env, MPMoviePlayerPlaybackDidFinishNotification);
            let _: () = msg![env; center postNotificationName:n4 object:sender];
        }

        release(env, inner);
        this
    }

    - (id)moviePlayer {
        // Return 'this' to trick the game into sending video commands to this object
        this
    }

- (())play {
        println!("🎮 LOG: Caught [MPMoviePlayerViewController play].");
        let center: id = msg_class![env; NSNotificationCenter defaultCenter];
        let finish_notif = crate::frameworks::foundation::ns_string::get_static_str(env, MPMoviePlayerPlaybackDidFinishNotification);
        let _: () = msg![env; center postNotificationName:finish_notif object:this];
    }

    - (())stop { }
    - (())pause { }
    - (())setControlStyle:(i32)_style { }
    - (())setScalingMode:(i32)_mode { }
    - (())setFullscreen:(bool)_fullscreen animated:(bool)_animated { }
    - (())setFullscreen:(bool)_fullscreen { }

    // ==========================================================
    // 🏎️ THE MOVIE CONFIGURATION GAUNTLET: Absorb everything!
    // ==========================================================
    - (())setMovieSourceType:(i32)source_type { }
    - (())setInitialPlaybackTime:(f64)time { }
    - (())setEndPlaybackTime:(f64)time { }
    - (())setShouldAutoplay:(bool)autoplay { }
    - (())setRepeatMode:(i32)mode { }

    - (i32)loadState { 3 }

    - (())setUseApplicationAudioSession:(bool)use_session { }

@end

};

/// For use by `NSRunLoop` via [super::handle_players]: check movie players'
/// status, send notifications if necessary.
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
        // TODO: should there be some user info attached?
        let _: () = msg![env; center postNotificationName:name object:object];
    }
}
