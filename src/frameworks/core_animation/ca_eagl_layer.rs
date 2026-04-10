/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CAEAGLLayer`.

use super::ca_layer::CALayerHostObject;
use crate::objc::{id, msg, msg_class, nil, objc_classes, Class, ClassExports};
use crate::Environment;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation CAEAGLLayer: CALayer

// EAGLDrawable implementation (the only one)

- (id)drawableProperties {
    // FIXME: do we need to return an empty dictionary rather than nil?
    env.objc.borrow::<CALayerHostObject>(this).drawable_properties
}

- (())setDrawableProperties:(id)props { // NSDictionary<NSString*, id>*
    let props: id = msg![env; props copy];
    env.objc.borrow_mut::<CALayerHostObject>(this).drawable_properties = props;
}

@end

};

/// If there is an opaque `CAEAGLLayer` that covers the entire screen, this
/// returns a pointer to it. Otherwise, it returns [nil].
///
/// To avoid a state management nightmare, we want to have an internal OpenGL ES
/// context for compositing, separate from any OpenGL ES contexts the app uses
/// for its rendering. When we have a `CAEAGLLayer` though, we need to transfer
/// a rendered frame from the app's context to the compositor's context, and
/// unfortunately the most practical way to do this is `glReadPixels()`, which
/// is highly inefficient. To make things efficient, then, we have a shortcut:
/// if the result of composition would be identical to the rendered frame, i.e.
/// there's a single full-screen layer, we skip transferring between contexts
/// and present it directly from the app's context. This function is used to
/// determine when that will happen.
pub fn find_fullscreen_eagl_layer(env: &mut Environment) -> id {
    if env.options.force_composition {
        return nil;
    }

    let windows = env.framework_state.uikit.ui_view.ui_window.windows.clone();
    // Assumes the windows in the list are ordered back-to-front.
    // TODO: this may not be correct once we support windowLevel.
    let mut top_window = windows
        .into_iter()
        .rev()
        .find(|&window| !msg![env; window isHidden])
        .unwrap_or(nil);

    // FallbackToHackWindow
    let hack_bits = *crate::libc::stdlib::HACK_MAIN_WINDOW.lock().unwrap();
    if top_window == nil && hack_bits != 0 {
        top_window = crate::mem::Ptr::from_bits(hack_bits);
    }

    if top_window == nil {
        return nil;
    }

    // RevertToLoop
    let mut layer: id = msg![env; top_window layer];
    //DebugFindLayer
    log!(
        "DEBUG_CAEAGL: find_fullscreen_eagl_layer START. top_window: {:?}",
        top_window
    );

    loop {
        assert!(layer != nil);

        let layer_host_obj: &CALayerHostObject = env.objc.borrow(layer);
        let b = layer_host_obj.bounds;
        //FixPackedStructLog
        let bx = b.origin.x;
        let by = b.origin.y;
        let bw = b.size.width;
        let bh = b.size.height;
        log!("DEBUG_CAEAGL: Inspecting layer: {:?} | bounds: x={},y={},w={},h={} | hidden: {}, opacity: {}", layer, bx, by, bw, bh, layer_host_obj.hidden, layer_host_obj.opacity);

        // BypassStrictBounds
        if layer_host_obj.hidden || layer_host_obj.opacity == 0.0 {
            log!("DEBUG_CAEAGL: Layer hidden/transparent, returning nil.");
            return nil;
        }

        if let Some(&next) = layer_host_obj.sublayers.last() {
            layer = next;
        } else {
            break;
        }
    }

    // IgnoreOpaqueFlag
    let ca_eagl_layer_class: Class = msg_class![env; CAEAGLLayer class];
    let is_eagl: bool = msg![env; layer isKindOfClass:ca_eagl_layer_class];
    let host: &CALayerHostObject = env.objc.borrow(layer);

    log!(
        "DEBUG_CAEAGL: Deepest layer: {:?} | is_eagl: {}, has_pixels: {}",
        layer,
        is_eagl,
        host.presented_pixels.is_some()
    );
    if !is_eagl && host.presented_pixels.is_none() {
        log!("DEBUG_CAEAGL: Not EAGL and no pixels, returning nil.");
        return nil;
    }

    log!("DEBUG_CAEAGL: Found valid fullscreen layer: {:?}", layer);
    layer
}

/// For use by `EAGLContext` when presenting to a `CAEAGLLayer`:
/// [std::mem::take]s the buffer used to hold the pixels. It should be passed
/// back to [present_pixels] once it has been filled.
pub fn get_pixels_vec_for_presenting(env: &mut Environment, layer: id) -> Vec<u8> {
    env.objc
        .borrow_mut::<CALayerHostObject>(layer)
        .presented_pixels
        .take()
        .map(|(vec, _width, _height)| vec)
        .unwrap_or_default()
}

/// For use by `EAGLContext` when presenting to a `CAEAGLLayer`: provide the new
/// frame rendered by the app, so it can be used when compositing. The buffer
/// should have been obtained with [get_pixels_vec_for_presenting] before
/// filling. The data must be in RGBA8 format.
pub fn present_pixels(env: &mut Environment, layer: id, pixels: Vec<u8>, width: u32, height: u32) {
    let host_obj = env.objc.borrow_mut::<CALayerHostObject>(layer);
    host_obj.presented_pixels = Some((pixels, width, height));
    host_obj.gles_texture_is_up_to_date = false;
}
