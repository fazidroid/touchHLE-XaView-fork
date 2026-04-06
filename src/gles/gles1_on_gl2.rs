/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Implementation of OpenGL ES 1.1 on top of OpenGL 2.1 compatibility profile.

use super::gl21compat_raw as gl21;
use super::gl21compat_raw::types::*;
use super::gles11_raw as gles11; // constants only
use super::gles_generic::GLES;
use super::util::{
    fixed_to_float, matrix_fixed_to_float, try_decode_pvrtc, PalettedTextureFormat, ParamTable,
    ParamType,
};
use super::GLESContext;
use crate::window::{GLContext, GLVersion, Window};
use std::collections::HashSet;
use std::ffi::CStr;

pub const CAPABILITIES: &[GLenum] = &[
    gl21::ALPHA_TEST, gl21::BLEND, gl21::COLOR_LOGIC_OP, gl21::CLIP_PLANE0, gl21::CLIP_PLANE1,
    gl21::CLIP_PLANE2, gl21::CLIP_PLANE3, gl21::CLIP_PLANE4, gl21::CLIP_PLANE5, gl21::LIGHT0,
    gl21::LIGHT1, gl21::LIGHT2, gl21::LIGHT3, gl21::LIGHT4, gl21::LIGHT5, gl21::LIGHT6,
    gl21::LIGHT7, gl21::COLOR_MATERIAL, gl21::CULL_FACE, gl21::DEPTH_TEST, gl21::DITHER,
    gl21::FOG, gl21::LIGHTING, gl21::LINE_SMOOTH, gl21::MULTISAMPLE, gl21::NORMALIZE,
    gl21::POINT_SMOOTH, gl21::POLYGON_OFFSET_FILL, gl21::RESCALE_NORMAL,
    gl21::SAMPLE_ALPHA_TO_COVERAGE, gl21::SAMPLE_ALPHA_TO_ONE, gl21::SAMPLE_COVERAGE,
    gl21::SCISSOR_TEST, gl21::STENCIL_TEST, gl21::TEXTURE_2D, gl21::POINT_SPRITE,
];

pub const UNSUPPORTED_CAPABILITIES: &[GLenum] = &[0x8620, gl21::TEXTURE];

pub struct ArrayInfo {
    pub name: GLenum,
    pub buffer_binding: GLenum,
    size: Option<GLenum>,
    stride: GLenum,
    pub pointer: GLenum,
}

struct ArrayStateBackup {
    size: Option<GLint>,
    stride: GLsizei,
    pointer: *const GLvoid,
    buffer_binding: GLuint,
}

pub const ARRAYS: &[ArrayInfo] = &[
    ArrayInfo {
        name: gl21::COLOR_ARRAY, buffer_binding: gl21::COLOR_ARRAY_BUFFER_BINDING,
        size: Some(gl21::COLOR_ARRAY_SIZE), stride: gl21::COLOR_ARRAY_STRIDE, pointer: gl21::COLOR_ARRAY_POINTER,
    },
    ArrayInfo {
        name: gl21::NORMAL_ARRAY, buffer_binding: gl21::NORMAL_ARRAY_BUFFER_BINDING,
        size: None, stride: gl21::NORMAL_ARRAY_STRIDE, pointer: gl21::NORMAL_ARRAY_POINTER,
    },
    ArrayInfo {
        name: gl21::TEXTURE_COORD_ARRAY, buffer_binding: gl21::TEXTURE_COORD_ARRAY_BUFFER_BINDING,
        size: Some(gl21::TEXTURE_COORD_ARRAY_SIZE), stride: gl21::TEXTURE_COORD_ARRAY_STRIDE, pointer: gl21::TEXTURE_COORD_ARRAY_POINTER,
    },
    ArrayInfo {
        name: gl21::VERTEX_ARRAY, buffer_binding: gl21::VERTEX_ARRAY_BUFFER_BINDING,
        size: Some(gl21::VERTEX_ARRAY_SIZE), stride: gl21::VERTEX_ARRAY_STRIDE, pointer: gl21::VERTEX_ARRAY_POINTER,
    },
];

const POINT_PARAMS: ParamTable = ParamTable(&[
    (gl21::POINT_SIZE_MIN, ParamType::Float, 1), (gl21::POINT_SIZE_MAX, ParamType::Float, 1),
    (gl21::POINT_DISTANCE_ATTENUATION, ParamType::Float, 3), (gl21::POINT_FADE_THRESHOLD_SIZE, ParamType::Float, 1),
    (gl21::POINT_SMOOTH, ParamType::Boolean, 1),
]);

const FOG_PARAMS: ParamTable = ParamTable(&[
    (gl21::FOG_MODE, ParamType::Int, 1), (gl21::FOG_DENSITY, ParamType::Float, 1),
    (gl21::FOG_START, ParamType::Float, 1), (gl21::FOG_END, ParamType::Float, 1),
    (gl21::FOG_COLOR, ParamType::FloatSpecial, 4),
]);

const LIGHT_PARAMS: ParamTable = ParamTable(&[
    (gl21::AMBIENT, ParamType::Float, 4), (gl21::DIFFUSE, ParamType::Float, 4),
    (gl21::SPECULAR, ParamType::Float, 4), (gl21::POSITION, ParamType::Float, 4),
    (gl21::SPOT_CUTOFF, ParamType::Float, 1), (gl21::SPOT_DIRECTION, ParamType::Float, 3),
    (gl21::SPOT_EXPONENT, ParamType::Float, 1), (gl21::CONSTANT_ATTENUATION, ParamType::Float, 1),
    (gl21::LINEAR_ATTENUATION, ParamType::Float, 1), (gl21::QUADRATIC_ATTENUATION, ParamType::Float, 1),
]);

const LIGHT_MODEL_PARAMS: ParamTable = ParamTable(&[
    (gl21::LIGHT_MODEL_AMBIENT, ParamType::Float, 4), (gl21::LIGHT_MODEL_TWO_SIDE, ParamType::Boolean, 1),
]);

const MATERIAL_PARAMS: ParamTable = ParamTable(&[
    (gl21::AMBIENT, ParamType::Float, 4), (gl21::DIFFUSE, ParamType::Float, 4),
    (gl21::SPECULAR, ParamType::Float, 4), (gl21::EMISSION, ParamType::Float, 4),
    (gl21::SHININESS, ParamType::Float, 1), (gl21::AMBIENT_AND_DIFFUSE, ParamType::Float, 4),
]);

const TEX_ENV_PARAMS: ParamTable = ParamTable(&[
    (gl21::TEXTURE_ENV_MODE, ParamType::Int, 1), (gl21::COORD_REPLACE, ParamType::Int, 1),
    (gl21::COMBINE_RGB, ParamType::Int, 1), (gl21::COMBINE_ALPHA, ParamType::Int, 1),
    (gl21::SRC0_RGB, ParamType::Int, 1), (gl21::SRC1_RGB, ParamType::Int, 1),
    (gl21::SRC2_RGB, ParamType::Int, 1), (gl21::SRC0_ALPHA, ParamType::Int, 1),
    (gl21::SRC1_ALPHA, ParamType::Int, 1), (gl21::SRC2_ALPHA, ParamType::Int, 1),
    (gl21::OPERAND0_RGB, ParamType::Int, 1), (gl21::OPERAND1_RGB, ParamType::Int, 1),
    (gl21::OPERAND2_RGB, ParamType::Int, 1), (gl21::OPERAND0_ALPHA, ParamType::Int, 1),
    (gl21::OPERAND1_ALPHA, ParamType::Int, 1), (gl21::OPERAND2_ALPHA, ParamType::Int, 1),
    (gl21::TEXTURE_ENV_COLOR, ParamType::Float, 4), (gl21::RGB_SCALE, ParamType::Float, 1),
    (gl21::ALPHA_SCALE, ParamType::Float, 1),
]);

const TEX_PARAMS: ParamTable = ParamTable(&[
    (gl21::TEXTURE_MIN_FILTER, ParamType::Int, 1), (gl21::TEXTURE_MAG_FILTER, ParamType::Int, 1),
    (gl21::TEXTURE_WRAP_S, ParamType::Int, 1), (gl21::TEXTURE_WRAP_T, ParamType::Int, 1),
    (gl21::GENERATE_MIPMAP, ParamType::Int, 1), (gl21::TEXTURE_MAX_ANISOTROPY_EXT, ParamType::Float, 1),
    (gl21::MAX_TEXTURE_MAX_ANISOTROPY_EXT, ParamType::Float, 1),
]);

pub struct GLES1OnGL2State {
    pointer_is_fixed_point: [bool; ARRAYS.len()],
    fixed_point_texture_units: HashSet<GLenum>,
    fixed_point_translation_buffers: [Vec<GLfloat>; ARRAYS.len()],
}

pub struct GLES1OnGL2Context {
    gl_ctx: GLContext,
    state: GLES1OnGL2State,
    is_loaded: bool,
}

impl GLESContext for GLES1OnGL2Context {
    fn description() -> &'static str { "OpenGL ES 1.1 via touchHLE GLES1-on-GL2 layer" }

    fn new(window: &mut Window, _options: &crate::options::Options) -> Result<Self, String> {
        Ok(Self {
            gl_ctx: window.create_gl_context(GLVersion::GL21Compat)?,
            state: GLES1OnGL2State {
                pointer_is_fixed_point: [false; ARRAYS.len()],
                fixed_point_texture_units: HashSet::new(),
                fixed_point_translation_buffers: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            },
            is_loaded: false,
        })
    }

    fn make_current<'gl_ctx, 'win: 'gl_ctx>(&'gl_ctx mut self, window: &'win mut Window) -> Box<dyn GLES + 'gl_ctx> {
        if self.gl_ctx.is_current() && self.is_loaded { return Box::new(GLES1OnGL2 { state: &mut self.state }); }
        unsafe { window.make_gl_context_current(&self.gl_ctx); }
        gl21::load_with(|s| window.gl_get_proc_address(s));
        self.is_loaded = true;
        Box::new(GLES1OnGL2 { state: &mut self.state })
    }

    unsafe fn make_current_unchecked_for_window<'gl_ctx>(&'gl_ctx mut self, make_current_fn: &mut dyn FnMut(&GLContext), loader_fn: &mut dyn FnMut(&'static str) -> *const std::ffi::c_void) -> Box<dyn GLES + 'gl_ctx> {
        if self.gl_ctx.is_current() && self.is_loaded { return Box::new(GLES1OnGL2 { state: &mut self.state }); }
        make_current_fn(&self.gl_ctx);
        gl21::load_with(loader_fn);
        self.is_loaded = true;
        Box::new(GLES1OnGL2 { state: &mut self.state })
    }
}

pub struct GLES1OnGL2<'a> {
    state: &'a mut GLES1OnGL2State,
}

impl GLES1OnGL2<'_> {
    unsafe fn translate_fixed_point_arrays(&mut self, first: GLint, count: GLsizei) -> [Option<ArrayStateBackup>; ARRAYS.len()] {
        let mut backups: [Option<ArrayStateBackup>; ARRAYS.len()] = Default::default();
        for (i, array_info) in ARRAYS.iter().enumerate() {
            if !self.state.pointer_is_fixed_point[i] { continue; }
            let old_client_active_texture = if array_info.name == gl21::TEXTURE_COORD_ARRAY {
                let mut active_texture: GLenum = 0;
                gl21::GetIntegerv(gl21::ACTIVE_TEXTURE, &mut active_texture as *mut _ as *mut _);
                if !self.state.fixed_point_texture_units.contains(&active_texture) { continue; }
                let mut old_client_active_texture: GLenum = 0;
                gl21::GetIntegerv(gl21::CLIENT_ACTIVE_TEXTURE, &mut old_client_active_texture as *mut _ as *mut _);
                gl21::ClientActiveTexture(active_texture);
                Some(old_client_active_texture)
            } else { None };

            let mut is_active = gl21::FALSE;
            gl21::GetBooleanv(array_info.name, &mut is_active);
            if is_active != gl21::TRUE { continue; }

            let mut buffer_binding = 0;
            gl21::GetIntegerv(array_info.buffer_binding, &mut buffer_binding);
            let size = array_info.size.map(|size_enum| {
                let mut size: GLint = 0;
                gl21::GetIntegerv(size_enum, &mut size);
                size
            });
            let mut stride: GLsizei = 0;
            gl21::GetIntegerv(array_info.stride, &mut stride);
            let old_pointer = {
                let mut pointer: *mut GLvoid = std::ptr::null_mut();
                #[allow(clippy::unnecessary_mut_passed)]
                gl21::GetPointerv(array_info.pointer, &mut pointer);
                pointer.cast_const()
            };
            backups[i] = Some(ArrayStateBackup { size, stride, pointer: old_pointer, buffer_binding: buffer_binding.try_into().unwrap() });
            let pointer = if buffer_binding != 0 {
                let mapped_buffer = gl21::MapBuffer(gl21::ARRAY_BUFFER, gl21::READ_ONLY);
                assert!(!mapped_buffer.is_null());
                mapped_buffer.add(old_pointer as usize)
            } else { old_pointer };

            let size = size.unwrap_or(3);
            let stride_val = if stride == 0 { size * 4 } else { stride };
            let buffer = &mut self.state.fixed_point_translation_buffers[i];
            buffer.clear();
            buffer.resize(((first + count) * size).try_into().unwrap(), 0.0);
            {
                let first = first as usize;
                let count = count as usize;
                let size = size as usize;
                let stride_v = stride_val as usize;
                for j in first..(first + count) {
                    let vector_ptr: *const GLfixed = pointer.add(j * stride_v).cast();
                    for k in 0..size {
                        buffer[j * size + k] = fixed_to_float(vector_ptr.add(k).read_unaligned());
                    }
                }
            }
            if buffer_binding != 0 { gl21::UnmapBuffer(gl21::ARRAY_BUFFER); gl21::BindBuffer(gl21::ARRAY_BUFFER, 0); }
            let buffer_ptr: *const GLvoid = buffer.as_ptr().cast();
            match array_info.name {
                gl21::COLOR_ARRAY => gl21::ColorPointer(size, gl21::FLOAT, 0, buffer_ptr),
                gl21::NORMAL_ARRAY => gl21::NormalPointer(gl21::FLOAT, 0, buffer_ptr),
                gl21::TEXTURE_COORD_ARRAY => gl21::TexCoordPointer(size, gl21::FLOAT, 0, buffer_ptr),
                gl21::VERTEX_ARRAY => gl21::VertexPointer(size, gl21::FLOAT, 0, buffer_ptr),
                _ => unreachable!(),
            }
            if let Some(old_client_active_texture) = old_client_active_texture { gl21::ClientActiveTexture(old_client_active_texture); }
        }
        backups
    }
    unsafe fn restore_fixed_point_arrays(&mut self, from_backup: [Option<ArrayStateBackup>; ARRAYS.len()]) {
        for (i, backup) in from_backup.into_iter().enumerate() {
            let Some(ArrayStateBackup { size, stride, pointer, buffer_binding }) = backup else { continue };
            if buffer_binding != 0 { gl21::BindBuffer(gl21::ARRAY_BUFFER, buffer_binding); }
            let array_info = &ARRAYS[i];
            match array_info.name {
                gl21::COLOR_ARRAY => gl21::ColorPointer(size.unwrap(), gl21::FLOAT, stride, pointer),
                gl21::NORMAL_ARRAY => gl21::NormalPointer(gl21::FLOAT, stride, pointer),
                gl21::TEXTURE_COORD_ARRAY => {
                    let mut old_client_active_texture: GLenum = 0;
                    gl21::GetIntegerv(gl21::CLIENT_ACTIVE_TEXTURE, &mut old_client_active_texture as *mut _ as *mut _);
                    gl21::TexCoordPointer(size.unwrap(), gl21::FLOAT, stride, pointer);
                    gl21::ClientActiveTexture(old_client_active_texture)
                }
                gl21::VERTEX_ARRAY => gl21::VertexPointer(size.unwrap(), gl21::FLOAT, stride, pointer),
                _ => unreachable!(),
            }
        }
    }
}

impl GLES for GLES1OnGL2<'_> {
    fn is_gles2(&self) -> bool { true }

    unsafe fn driver_description(&self) -> String {
        let version = CStr::from_ptr(gl21::GetString(gl21::VERSION) as *const _);
        let vendor = CStr::from_ptr(gl21::GetString(gl21::VENDOR) as *const _);
        let renderer = CStr::from_ptr(gl21::GetString(gl21::RENDERER) as *const _);
        format!("OpenGL {} / {} / {}", version.to_string_lossy(), vendor.to_string_lossy(), renderer.to_string_lossy())
    }

    unsafe fn GetIntegerv(&mut self, pname: GLenum, params: *mut GLint) {
        // FIXED: Intercept shader binary format queries to prevent 0x8df9 panics
        if pname == 0x8df8 { *params = 0; return; }
        if pname == 0x8df9 { return; }
        
        // FIXED: Bypass GET_PARAMS validation to allow native ES 2.0 queries from N.O.V.A. 3
        gl21::GetIntegerv(pname, params);
    }

    unsafe fn GetError(&mut self) -> GLenum { gl21::GetError() }
    unsafe fn Enable(&mut self, cap: GLenum) { gl21::Enable(cap); }
    unsafe fn IsEnabled(&mut self, cap: GLenum) -> GLboolean { gl21::IsEnabled(cap) }
    unsafe fn Disable(&mut self, cap: GLenum) { gl21::Disable(cap); }
    unsafe fn ClientActiveTexture(&mut self, texture: GLenum) { gl21::ClientActiveTexture(texture); }
    unsafe fn EnableClientState(&mut self, array: GLenum) { gl21::EnableClientState(array); }
    unsafe fn DisableClientState(&mut self, array: GLenum) { gl21::DisableClientState(array); }
    unsafe fn GetBooleanv(&mut self, pname: GLenum, params: *mut GLboolean) { gl21::GetBooleanv(pname, params); }
    unsafe fn GetFloatv(&mut self, pname: GLenum, params: *mut GLfloat) { gl21::GetFloatv(pname, params); }
    unsafe fn GetTexEnviv(&mut self, target: GLenum, pname: GLenum, params: *mut GLint) { gl21::GetTexEnviv(target, pname, params); }
    unsafe fn GetTexEnvfv(&mut self, target: GLenum, pname: GLenum, params: *mut GLfloat) { gl21::GetTexEnvfv(target, pname, params); }
    unsafe fn GetPointerv(&mut self, pname: GLenum, params: *mut *const GLvoid) { gl21::GetPointerv(pname, params as *mut _ as *const _); }
    unsafe fn Hint(&mut self, target: GLenum, mode: GLenum) { gl21::Hint(target, mode); }
    unsafe fn Finish(&mut self) { gl21::Finish(); }
    unsafe fn Flush(&mut self) { gl21::Flush(); }
    unsafe fn GetString(&mut self, name: GLenum) -> *const GLubyte { gl21::GetString(name) }
    unsafe fn AlphaFunc(&mut self, func: GLenum, ref_: GLclampf) { gl21::AlphaFunc(func, ref_); }
    unsafe fn AlphaFuncx(&mut self, func: GLenum, ref_: GLclampx) { self.AlphaFunc(func, fixed_to_float(ref_)); }
    unsafe fn BlendFunc(&mut self, sfactor: GLenum, dfactor: GLenum) { gl21::BlendFunc(sfactor, dfactor); }
    unsafe fn BlendEquationOES(&mut self, mode: GLenum) { gl21::BlendEquation(mode); }
    unsafe fn ColorMask(&mut self, r: GLboolean, g: GLboolean, b: GLboolean, a: GLboolean) { gl21::ColorMask(r, g, b, a); }
    unsafe fn ClipPlanef(&mut self, plane: GLenum, eq: *const GLfloat) { let mut eq_d: [GLdouble; 4] = [0.0; 4]; for i in 0..4 { eq_d[i] = unsafe { *eq.add(i) } as GLdouble; } gl21::ClipPlane(plane, eq_d.as_ptr()); }
    unsafe fn ClipPlanex(&mut self, plane: GLenum, eq: *const GLfixed) { let mut eq_d: [GLdouble; 4] = [0.0; 4]; for i in 0..4 { eq_d[i] = fixed_to_float(unsafe { *eq.add(i) }) as GLdouble; } gl21::ClipPlane(plane, eq_d.as_ptr()); }
    unsafe fn CullFace(&mut self, mode: GLenum) { gl21::CullFace(mode); }
    unsafe fn DepthFunc(&mut self, func: GLenum) { gl21::DepthFunc(func); }
    unsafe fn DepthMask(&mut self, flag: GLboolean) { gl21::DepthMask(flag); }
    unsafe fn FrontFace(&mut self, mode: GLenum) { gl21::FrontFace(mode); }
    unsafe fn DepthRangef(&mut self, n: GLclampf, f: GLclampf) { gl21::DepthRange(n.into(), f.into()); }
    unsafe fn DepthRangex(&mut self, n: GLclampx, f: GLclampx) { gl21::DepthRange(fixed_to_float(n).into(), fixed_to_float(f).into()); }
    unsafe fn PolygonOffset(&mut self, f: GLfloat, u: GLfloat) { gl21::PolygonOffset(f, u); }
    unsafe fn PolygonOffsetx(&mut self, f: GLfixed, u: GLfixed) { gl21::PolygonOffset(fixed_to_float(f), fixed_to_float(u)); }
    unsafe fn SampleCoverage(&mut self, v: GLclampf, i: GLboolean) { gl21::SampleCoverage(v, i); }
    unsafe fn SampleCoveragex(&mut self, v: GLclampx, i: GLboolean) { gl21::SampleCoverage(fixed_to_float(v), i); }
    unsafe fn ShadeModel(&mut self, m: GLenum) { gl21::ShadeModel(m); }
    unsafe fn Scissor(&mut self, x: GLint, y: GLint, w: GLsizei, h: GLsizei) { gl21::Scissor(x, y, w, h); }
    unsafe fn Viewport(&mut self, x: GLint, y: GLint, w: GLsizei, h: GLsizei) { gl21::Viewport(x, y, w, h); }
    unsafe fn LineWidth(&mut self, v: GLfloat) { gl21::LineWidth(v); }
    unsafe fn LineWidthx(&mut self, v: GLfixed) { gl21::LineWidth(fixed_to_float(v)); }
    unsafe fn StencilFunc(&mut self, f: GLenum, r: GLint, m: GLuint) { gl21::StencilFunc(f, r, m); }
    unsafe fn StencilOp(&mut self, s: GLenum, d1: GLenum, d2: GLenum) { gl21::StencilOp(s, d1, d2); }
    unsafe fn StencilMask(&mut self, m: GLuint) { gl21::StencilMask(m); }
    unsafe fn LogicOp(&mut self, o: GLenum) { gl21::LogicOp(o); }
    unsafe fn PointSize(&mut self, s: GLfloat) { gl21::PointSize(s); }
    unsafe fn PointSizex(&mut self, s: GLfixed) { gl21::PointSize(fixed_to_float(s)); }
    unsafe fn PointParameterf(&mut self, p: GLenum, v: GLfloat) { gl21::PointParameterf(p, v); }
    unsafe fn PointParameterfv(&mut self, p: GLenum, v: *const GLfloat) { gl21::PointParameterfv(p, v); }
    unsafe fn PointParameterx(&mut self, p: GLenum, v: GLfixed) { POINT_PARAMS.setx(|v| gl21::PointParameterf(p, v), |_| {}, p, v); }
    unsafe fn PointParameterxv(&mut self, p: GLenum, v: *const GLfixed) { POINT_PARAMS.setxv(|v| gl21::PointParameterfv(p, v), |_| {}, p, v); }
    unsafe fn Fogf(&mut self, p: GLenum, v: GLfloat) { gl21::Fogf(p, v); }
    unsafe fn Fogx(&mut self, p: GLenum, v: GLfixed) { FOG_PARAMS.setx(|v| gl21::Fogf(p, v), |v| gl21::Fogi(p, v), p, v); }
    unsafe fn Fogfv(&mut self, p: GLenum, v: *const GLfloat) { gl21::Fogfv(p, v); }
    unsafe fn Fogxv(&mut self, p: GLenum, v: *const GLfixed) { FOG_PARAMS.setxv(|v| gl21::Fogfv(p, v), |v| gl21::Fogiv(p, v), p, v); }
    unsafe fn Lightf(&mut self, l: GLenum, p: GLenum, v: GLfloat) { gl21::Lightf(l, p, v); }
    unsafe fn Lightx(&mut self, l: GLenum, p: GLenum, v: GLfixed) { LIGHT_PARAMS.setx(|v| gl21::Lightf(l, p, v), |v| gl21::Lighti(l, p, v), p, v); }
    unsafe fn Lightfv(&mut self, l: GLenum, p: GLenum, v: *const GLfloat) { gl21::Lightfv(l, p, v); }
    unsafe fn Lightxv(&mut self, l: GLenum, p: GLenum, v: *const GLfixed) { LIGHT_PARAMS.setxv(|v| gl21::Lightfv(l, p, v), |v| gl21::Lightiv(l, p, v), p, v); }
    unsafe fn LightModelf(&mut self, p: GLenum, v: GLfloat) { gl21::LightModelf(p, v); }
    unsafe fn LightModelx(&mut self, p: GLenum, v: GLfixed) { LIGHT_MODEL_PARAMS.setx(|v| gl21::LightModelf(p, v), |v| gl21::LightModeli(p, v), p, v); }
    unsafe fn LightModelfv(&mut self, p: GLenum, v: *const GLfloat) { gl21::LightModelfv(p, v); }
    unsafe fn LightModelxv(&mut self, p: GLenum, v: *const GLfixed) { LIGHT_MODEL_PARAMS.setxv(|v| gl21::LightModelfv(p, v), |v| gl21::LightModeliv(p, v), p, v); }
    unsafe fn Materialf(&mut self, f: GLenum, p: GLenum, v: GLfloat) { gl21::Materialf(f, p, v); }
    unsafe fn Materialx(&mut self, f: GLenum, p: GLenum, v: GLfixed) { MATERIAL_PARAMS.setx(|v| gl21::Materialf(f, p, v), |_| {}, p, v); }
    unsafe fn Materialfv(&mut self, f: GLenum, p: GLenum, v: *const GLfloat) { gl21::Materialfv(f, p, v); }
    unsafe fn Materialxv(&mut self, f: GLenum, p: GLenum, v: *const GLfixed) { MATERIAL_PARAMS.setxv(|v| gl21::Materialfv(f, p, v), |_| {}, p, v); }
    unsafe fn IsBuffer(&mut self, b: GLuint) -> GLboolean { gl21::IsBuffer(b) }
    unsafe fn GenBuffers(&mut self, n: GLsizei, b: *mut GLuint) { gl21::GenBuffers(n, b); }
    unsafe fn DeleteBuffers(&mut self, n: GLsizei, b: *const GLuint) { gl21::DeleteBuffers(n, b); }
    unsafe fn BindBuffer(&mut self, t: GLenum, b: GLuint) { gl21::BindBuffer(t, b); }
    unsafe fn BufferData(&mut self, t: GLenum, s: GLsizeiptr, d: *const GLvoid, u: GLenum) { gl21::BufferData(t, s, d, u); }
    unsafe fn BufferSubData(&mut self, t: GLenum, o: GLintptr, s: GLsizeiptr, d: *const GLvoid) { gl21::BufferSubData(t, o, s, d); }
    unsafe fn Color4f(&mut self, r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat) { gl21::Color4f(r, g, b, a); }
    unsafe fn Color4x(&mut self, r: GLfixed, g: GLfixed, b: GLfixed, a: GLfixed) { gl21::Color4f(fixed_to_float(r), fixed_to_float(g), fixed_to_float(b), fixed_to_float(a)); }
    unsafe fn Color4ub(&mut self, r: GLubyte, g: GLubyte, b: GLubyte, a: GLubyte) { gl21::Color4ub(r, g, b, a); }
    unsafe fn Normal3f(&mut self, x: GLfloat, y: GLfloat, z: GLfloat) { gl21::Normal3f(x, y, z); }
    unsafe fn Normal3x(&mut self, x: GLfixed, y: GLfixed, z: GLfixed) { gl21::Normal3f(fixed_to_float(x), fixed_to_float(y), fixed_to_float(z)); }
    unsafe fn ColorPointer(&mut self, s: GLint, t: GLenum, st: GLsizei, p: *const GLvoid) { self.state.pointer_is_fixed_point[0] = t == gles11::FIXED; gl21::ColorPointer(s, if t == gles11::FIXED { gl21::FLOAT } else { t }, st, p); }
    unsafe fn NormalPointer(&mut self, t: GLenum, st: GLsizei, p: *const GLvoid) { self.state.pointer_is_fixed_point[1] = t == gles11::FIXED; gl21::NormalPointer(if t == gles11::FIXED { gl21::FLOAT } else { t }, st, p); }
    unsafe fn TexCoordPointer(&mut self, s: GLint, t: GLenum, st: GLsizei, p: *const GLvoid) { let mut active: GLenum = 0; gl21::GetIntegerv(gl21::CLIENT_ACTIVE_TEXTURE, &mut active as *mut _ as *mut _); if t == gles11::FIXED { self.state.fixed_point_texture_units.insert(active); self.state.pointer_is_fixed_point[2] = true; } else { self.state.fixed_point_texture_units.remove(&active); if self.state.fixed_point_texture_units.is_empty() { self.state.pointer_is_fixed_point[2] = false; } } gl21::TexCoordPointer(s, if t == gles11::FIXED { gl21::FLOAT } else { t }, st, p); }
    unsafe fn VertexPointer(&mut self, s: GLint, t: GLenum, st: GLsizei, p: *const GLvoid) { self.state.pointer_is_fixed_point[3] = t == gles11::FIXED; gl21::VertexPointer(s, if t == gles11::FIXED { gl21::FLOAT } else { t }, st, p); }

    unsafe fn DrawArrays(&mut self, mode: GLenum, first: GLint, count: GLsizei) {
        let backup = self.translate_fixed_point_arrays(first, count);
        gl21::DrawArrays(mode, first, count);
        self.restore_fixed_point_arrays(backup);
    }
    unsafe fn DrawElements(&mut self, m: GLenum, c: GLsizei, t: GLenum, i: *const GLvoid) {
        let backup = if self.state.pointer_is_fixed_point.iter().any(|&v| v) {
            let mut first = usize::MAX; let mut last = usize::MIN;
            match t {
                gl21::UNSIGNED_BYTE => { let p = i.cast::<GLubyte>(); for idx in 0..(c as usize) { let v = unsafe { *p.add(idx) } as usize; first = first.min(v); last = last.max(v); } }
                gl21::UNSIGNED_SHORT => { let p = i.cast::<GLushort>(); for idx in 0..(c as usize) { let v = unsafe { *p.add(idx) } as usize; first = first.min(v); last = last.max(v); } }
                _ => unreachable!(),
            }
            Some(self.translate_fixed_point_arrays(first as GLint, (last + 1 - first) as GLsizei))
        } else { None };
        gl21::DrawElements(m, c, t, i);
        if let Some(b) = backup { self.restore_fixed_point_arrays(b); }
    }

    unsafe fn Clear(&mut self, m: GLbitfield) { gl21::Clear(m); }
    unsafe fn ClearColor(&mut self, r: GLclampf, g: GLclampf, b: GLclampf, a: GLclampf) { gl21::ClearColor(r, g, b, a); }
    unsafe fn ClearColorx(&mut self, r: GLclampx, g: GLclampx, b: GLclampx, a: GLclampx) { gl21::ClearColor(fixed_to_float(r), fixed_to_float(g), fixed_to_float(b), fixed_to_float(a)); }
    unsafe fn ClearDepthf(&mut self, d: GLclampf) { gl21::ClearDepth(d.into()); }
    unsafe fn ClearDepthx(&mut self, d: GLclampx) { self.ClearDepthf(fixed_to_float(d)); }
    unsafe fn ClearStencil(&mut self, s: GLint) { gl21::ClearStencil(s); }
    unsafe fn PixelStorei(&mut self, p: GLenum, v: GLint) { gl21::PixelStorei(p, v); }
    unsafe fn ReadPixels(&mut self, x: GLint, y: GLint, w: GLsizei, h: GLsizei, f: GLenum, t: GLenum, p: *mut GLvoid) { gl21::ReadPixels(x, y, w, h, f, t, p); }
    unsafe fn GenTextures(&mut self, n: GLsizei, t: *mut GLuint) { gl21::GenTextures(n, t); }
    unsafe fn DeleteTextures(&mut self, n: GLsizei, t: *const GLuint) { gl21::DeleteTextures(n, t); }
    unsafe fn ActiveTexture(&mut self, t: GLenum) { gl21::ActiveTexture(t); }
    unsafe fn IsTexture(&mut self, t: GLuint) -> GLboolean { gl21::IsTexture(t) }
    unsafe fn BindTexture(&mut self, target: GLenum, texture: GLuint) { gl21::BindTexture(target, texture); }
    unsafe fn TexParameteri(&mut self, t: GLenum, p: GLenum, v: GLint) { gl21::TexParameteri(t, p, v); }
    unsafe fn TexParameterf(&mut self, t: GLenum, p: GLenum, v: GLfloat) { gl21::TexParameterf(t, p, v); }
    unsafe fn TexParameterx(&mut self, t: GLenum, p: GLenum, v: GLfixed) { TEX_PARAMS.setx(|v| gl21::TexParameterf(t, p, v), |v| gl21::TexParameteri(t, p, v), p, v); }
    unsafe fn TexParameteriv(&mut self, t: GLenum, p: GLenum, v: *const GLint) { gl21::TexParameteriv(t, p, v); }
    unsafe fn TexParameterfv(&mut self, t: GLenum, p: GLenum, v: *const GLfloat) { gl21::TexParameterfv(t, p, v); }
    unsafe fn TexParameterxv(&mut self, t: GLenum, p: GLenum, v: *const GLfixed) { TEX_PARAMS.setxv(|v| gl21::TexParameterfv(t, p, v), |v| gl21::TexParameteriv(t, p, v), p, v); }
    unsafe fn TexImage2D(&mut self, t: GLenum, l: GLint, i: GLint, w: GLsizei, h: GLsizei, b: GLint, f: GLenum, ty: GLenum, p: *const GLvoid) { gl21::TexImage2D(t, l, i, w, h, b, f, ty, p); }
    unsafe fn TexSubImage2D(&mut self, t: GLenum, l: GLint, x: GLint, y: GLint, w: GLsizei, h: GLsizei, f: GLenum, ty: GLenum, p: *const GLvoid) { gl21::TexSubImage2D(t, l, x, y, w, h, f, ty, p); }
    
    unsafe fn CompressedTexImage2D(&mut self, t: GLenum, l: GLint, i: GLenum, w: GLsizei, h: GLsizei, b: GLint, s: GLsizei, d: *const GLvoid) { 
        let data = unsafe { std::slice::from_raw_parts(d.cast::<u8>(), s as usize) }; 
        if try_decode_pvrtc(self, t, l, i, w, h, b, data) { return; } 
        
        if let Some(PalettedTextureFormat { index_is_nibble, palette_entry_format, palette_entry_type }) = PalettedTextureFormat::get_info(i) {
            let palette_entry_size = match palette_entry_type {
                gl21::UNSIGNED_BYTE => match palette_entry_format { gl21::RGB => 3, gl21::RGBA => 4, _ => unreachable!() },
                gl21::UNSIGNED_SHORT_5_6_5 | gl21::UNSIGNED_SHORT_4_4_4_4 | gl21::UNSIGNED_SHORT_5_5_5_1 => 2,
                _ => unreachable!(),
            };
            let palette_entry_count = if index_is_nibble { 16 } else { 256 };
            let palette_size = palette_entry_size * palette_entry_count;
            let index_count = w as usize * h as usize;
            let (index_word_size, index_word_count) = if index_is_nibble { (1, index_count.div_ceil(2)) } else { (4, index_count.div_ceil(4)) };
            let _indices_size = index_word_size * index_word_count; // FIXED: Added underscore to silence warning
            let (palette, indices) = data.split_at(palette_size);
            let mut decoded = Vec::<u8>::with_capacity(palette_entry_size * index_count);
            for idx in 0..index_count {
                let index = if index_is_nibble { (indices[idx / 2] >> ((1 - (idx % 2)) * 4)) & 0xf } else { indices[idx] } as usize;
                decoded.extend_from_slice(&palette[index * palette_entry_size..][..palette_entry_size]);
            }
            gl21::TexImage2D(t, l, palette_entry_format as _, w, h, b, palette_entry_format, palette_entry_type, decoded.as_ptr() as *const _);
            return;
        }
        unimplemented!("CompressedTexImage2D internalformat: {:#x}", i); 
    }

    unsafe fn CopyTexImage2D(&mut self, t: GLenum, l: GLint, i: GLenum, x: GLint, y: GLint, w: GLsizei, h: GLsizei, b: GLint) { gl21::CopyTexImage2D(t, l, i, x, y, w, h, b); }
    unsafe fn CopyTexSubImage2D(&mut self, t: GLenum, l: GLint, x: GLint, y: GLint, sx: GLint, sy: GLint, w: GLsizei, h: GLsizei) { gl21::CopyTexSubImage2D(t, l, x, y, sx, sy, w, h); }
    unsafe fn TexEnvf(&mut self, t: GLenum, p: GLenum, v: GLfloat) { gl21::TexEnvf(t, p, v); }
    unsafe fn TexEnvi(&mut self, t: GLenum, p: GLenum, v: GLint) { gl21::TexEnvi(t, p, v); }
    unsafe fn TexEnvx(&mut self, t: GLenum, p: GLenum, v: GLfixed) { if t == gl21::TEXTURE_ENV { TEX_ENV_PARAMS.setx(|v| gl21::TexEnvf(t, p, v), |v| gl21::TexEnvi(t, p, v), p, v); } else { gl21::TexEnvf(t, p, fixed_to_float(v)); } }
    unsafe fn TexEnvfv(&mut self, t: GLenum, p: GLenum, v: *const GLfloat) { gl21::TexEnvfv(t, p, v); }
    unsafe fn TexEnvxv(&mut self, t: GLenum, p: GLenum, v: *const GLfixed) { if t == gl21::TEXTURE_ENV { TEX_ENV_PARAMS.setxv(|v| gl21::TexEnvfv(t, p, v), |v| gl21::TexEnviv(t, p, v), p, v); } else { let param = fixed_to_float(unsafe { *v }); gl21::TexEnvfv(t, p, &param); } }
    unsafe fn TexEnviv(&mut self, t: GLenum, p: GLenum, v: *const GLint) { gl21::TexEnviv(t, p, v); }
    unsafe fn MultiTexCoord4f(&mut self, t: GLenum, s: GLfloat, tc: GLfloat, r: GLfloat, q: GLfloat) { gl21::MultiTexCoord4f(t, s, tc, r, q); }
    unsafe fn MultiTexCoord4x(&mut self, t: GLenum, s: GLfixed, tc: GLfixed, r: GLfixed, q: GLfixed) { gl21::MultiTexCoord4f(t, fixed_to_float(s), fixed_to_float(tc), fixed_to_float(r), fixed_to_float(q)); }
    unsafe fn MatrixMode(&mut self, m: GLenum) { gl21::MatrixMode(m); }
    unsafe fn LoadIdentity(&mut self) { gl21::LoadIdentity(); }
    unsafe fn LoadMatrixf(&mut self, m: *const GLfloat) { gl21::LoadMatrixf(m); }
    unsafe fn LoadMatrixx(&mut self, m: *const GLfixed) { gl21::LoadMatrixf(matrix_fixed_to_float(m).as_ptr()); }
    unsafe fn MultMatrixf(&mut self, m: *const GLfloat) { gl21::MultMatrixf(m); }
    unsafe fn MultMatrixx(&mut self, m: *const GLfixed) { gl21::MultMatrixf(matrix_fixed_to_float(m).as_ptr()); }
    unsafe fn PushMatrix(&mut self) { gl21::PushMatrix(); }
    unsafe fn PopMatrix(&mut self) { gl21::PopMatrix(); }
    unsafe fn Orthof(&mut self, l: GLfloat, r: GLfloat, b: GLfloat, t: GLfloat, n: GLfloat, f: GLfloat) { gl21::Ortho(l.into(), r.into(), b.into(), t.into(), n.into(), f.into()); }
    unsafe fn Orthox(&mut self, l: GLfixed, r: GLfixed, b: GLfixed, t: GLfixed, n: GLfixed, f: GLfixed) { gl21::Ortho(fixed_to_float(l).into(), fixed_to_float(r).into(), fixed_to_float(b).into(), fixed_to_float(t).into(), fixed_to_float(n).into(), fixed_to_float(f).into()); }
    unsafe fn Frustumf(&mut self, l: GLfloat, r: GLfloat, b: GLfloat, t: GLfloat, n: GLfloat, f: GLfloat) { gl21::Frustum(l.into(), r.into(), b.into(), t.into(), n.into(), f.into()); }
    unsafe fn Frustumx(&mut self, l: GLfixed, r: GLfixed, b: GLfixed, t: GLfixed, n: GLfixed, f: GLfixed) { gl21::Frustum(fixed_to_float(l).into(), fixed_to_float(r).into(), fixed_to_float(b).into(), fixed_to_float(t).into(), fixed_to_float(n).into(), fixed_to_float(f).into()); }
    unsafe fn Rotatef(&mut self, a: GLfloat, x: GLfloat, y: GLfloat, z: GLfloat) { gl21::Rotatef(a, x, y, z); }
    unsafe fn Rotatex(&mut self, a: GLfixed, x: GLfixed, y: GLfixed, z: GLfixed) { gl21::Rotatef(fixed_to_float(a), fixed_to_float(x), fixed_to_float(y), fixed_to_float(z)); }
    unsafe fn Scalef(&mut self, x: GLfloat, y: GLfloat, z: GLfloat) { gl21::Scalef(x, y, z); }
    unsafe fn Scalex(&mut self, x: GLfixed, y: GLfixed, z: GLfixed) { gl21::Scalef(fixed_to_float(x), fixed_to_float(y), fixed_to_float(z)); }
    unsafe fn Translatef(&mut self, x: GLfloat, y: GLfloat, z: GLfloat) { gl21::Translatef(x, y, z); }
    unsafe fn Translatex(&mut self, x: GLfixed, y: GLfixed, z: GLfixed) { gl21::Translatef(fixed_to_float(x), fixed_to_float(y), fixed_to_float(z)); }

    unsafe fn CreateShader(&mut self, t: GLenum) -> GLuint { gl21::CreateShader(t) }
    unsafe fn ShaderSource(&mut self, s: GLuint, c: GLsizei, st: *const *const std::ffi::c_char, l: *const GLint) { gl21::ShaderSource(s, c, st, l) }
    unsafe fn CompileShader(&mut self, s: GLuint) { gl21::CompileShader(s) }
    unsafe fn DeleteShader(&mut self, s: GLuint) { gl21::DeleteShader(s) }
    unsafe fn GetShaderiv(&mut self, s: GLuint, p: GLenum, ps: *mut GLint) { gl21::GetShaderiv(s, p, ps) }
    unsafe fn GetShaderInfoLog(&mut self, s: GLuint, b: GLsizei, l: *mut GLsizei, i: *mut std::ffi::c_char) { gl21::GetShaderInfoLog(s, b, l, i) }
    unsafe fn CreateProgram(&mut self) -> GLuint { gl21::CreateProgram() }
    unsafe fn DeleteProgram(&mut self, p: GLuint) { gl21::DeleteProgram(p) }
    unsafe fn AttachShader(&mut self, p: GLuint, s: GLuint) { gl21::AttachShader(p, s) }
    unsafe fn BindAttribLocation(&mut self, p: GLuint, i: GLuint, n: *const std::ffi::c_char) { gl21::BindAttribLocation(p, i, n) }
    unsafe fn LinkProgram(&mut self, p: GLuint) { gl21::LinkProgram(p) }
    unsafe fn UseProgram(&mut self, p: GLuint) { gl21::UseProgram(p) }
    unsafe fn GetProgramiv(&mut self, p: GLuint, pn: GLenum, ps: *mut GLint) { gl21::GetProgramiv(p, pn, ps) }
    unsafe fn GetProgramInfoLog(&mut self, p: GLuint, b: GLsizei, l: *mut GLsizei, i: *mut std::ffi::c_char) { gl21::GetProgramInfoLog(p, b, l, i) }
    unsafe fn VertexAttribPointer(&mut self, i: GLuint, s: GLint, t: GLenum, n: GLboolean, st: GLsizei, p: *const GLvoid) { gl21::VertexAttribPointer(i, s, t, n, st, p) }
    unsafe fn DisableVertexAttribArray(&mut self, i: GLuint) { gl21::DisableVertexAttribArray(i) }
    unsafe fn EnableVertexAttribArray(&mut self, i: GLuint) { gl21::EnableVertexAttribArray(i) }
    
    unsafe fn VertexAttrib1f(&mut self, indx: GLuint, x: GLfloat) { gl21::VertexAttrib1f(indx, x) }
    unsafe fn VertexAttrib2f(&mut self, indx: GLuint, x: GLfloat, y: GLfloat) { gl21::VertexAttrib2f(indx, x, y) }
    unsafe fn VertexAttrib3f(&mut self, indx: GLuint, x: GLfloat, y: GLfloat, z: GLfloat) { gl21::VertexAttrib3f(indx, x, y, z) }
    unsafe fn VertexAttrib4f(&mut self, i: GLuint, x: GLfloat, y: GLfloat, z: GLfloat, w: GLfloat) { gl21::VertexAttrib4f(i, x, y, z, w) }
    unsafe fn VertexAttrib1fv(&mut self, indx: GLuint, values: *const GLfloat) { gl21::VertexAttrib1fv(indx, values) }
    unsafe fn VertexAttrib2fv(&mut self, indx: GLuint, values: *const GLfloat) { gl21::VertexAttrib2fv(indx, values) }
    unsafe fn VertexAttrib3fv(&mut self, indx: GLuint, values: *const GLfloat) { gl21::VertexAttrib3fv(indx, values) }
    unsafe fn VertexAttrib4fv(&mut self, indx: GLuint, values: *const GLfloat) { gl21::VertexAttrib4fv(indx, values) }
    unsafe fn Uniform1i(&mut self, l: GLint, v: GLint) { gl21::Uniform1i(l, v) }
    unsafe fn Uniform1f(&mut self, l: GLint, v: GLfloat) { gl21::Uniform1f(l, v) }
    unsafe fn Uniform2f(&mut self, l: GLint, v0: GLfloat, v1: GLfloat) { gl21::Uniform2f(l, v0, v1) }
    unsafe fn Uniform3f(&mut self, l: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat) { gl21::Uniform3f(l, v0, v1, v2) }
    unsafe fn Uniform4f(&mut self, l: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat, v3: GLfloat) { gl21::Uniform4f(l, v0, v1, v2, v3) }
    unsafe fn Uniform1fv(&mut self, l: GLint, c: GLsizei, v: *const GLfloat) { gl21::Uniform1fv(l, c, v) }
    unsafe fn Uniform2fv(&mut self, l: GLint, c: GLsizei, v: *const GLfloat) { gl21::Uniform2fv(l, c, v) }
    unsafe fn Uniform3fv(&mut self, l: GLint, c: GLsizei, v: *const GLfloat) { gl21::Uniform3fv(l, c, v) }
    unsafe fn Uniform4fv(&mut self, l: GLint, c: GLsizei, v: *const GLfloat) { gl21::Uniform4fv(l, c, v) }
    unsafe fn Uniform1iv(&mut self, location: GLint, count: GLsizei, value: *const GLint) { gl21::Uniform1iv(location, count, value) }
    unsafe fn Uniform2iv(&mut self, location: GLint, count: GLsizei, value: *const GLint) { gl21::Uniform2iv(location, count, value) }
    unsafe fn Uniform3iv(&mut self, location: GLint, count: GLsizei, value: *const GLint) { gl21::Uniform3iv(location, count, value) }
    unsafe fn Uniform4iv(&mut self, location: GLint, count: GLsizei, value: *const GLint) { gl21::Uniform4iv(location, count, value) }
    unsafe fn UniformMatrix2fv(&mut self, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { gl21::UniformMatrix2fv(location, count, transpose, value) }
    unsafe fn UniformMatrix3fv(&mut self, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { gl21::UniformMatrix3fv(location, count, transpose, value) }
    unsafe fn UniformMatrix4fv(&mut self, l: GLint, c: GLsizei, t: GLboolean, v: *const GLfloat) { gl21::UniformMatrix4fv(l, c, t, v) }
    unsafe fn GetUniformLocation(&mut self, p: GLuint, n: *const std::ffi::c_char) -> GLint { gl21::GetUniformLocation(p, n) }
    unsafe fn GetAttribLocation(&mut self, p: GLuint, n: *const std::ffi::c_char) -> GLint { gl21::GetAttribLocation(p, n) }
    unsafe fn GetActiveUniform(&mut self, program: GLuint, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, size: *mut GLint, type_: *mut GLenum, name: *mut std::ffi::c_char) { gl21::GetActiveUniform(program, index, bufSize, length, size, type_, name) }
    unsafe fn GetActiveAttrib(&mut self, program: GLuint, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, size: *mut GLint, type_: *mut GLenum, name: *mut std::ffi::c_char) { gl21::GetActiveAttrib(program, index, bufSize, length, size, type_, name) }
    unsafe fn BlendColor(&mut self, red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) { gl21::BlendColor(red, green, blue, alpha) }
    unsafe fn GetVertexAttribiv(&mut self, index: GLuint, pname: GLenum, params: *mut GLint) { gl21::GetVertexAttribiv(index, pname, params) }

    unsafe fn GenFramebuffersOES(&mut self, n: GLsizei, f: *mut GLuint) { gl21::GenFramebuffersEXT(n, f) }
    unsafe fn GenRenderbuffersOES(&mut self, n: GLsizei, r: *mut GLuint) { gl21::GenRenderbuffersEXT(n, r) }
    unsafe fn IsFramebufferOES(&mut self, r: GLuint) -> GLboolean { gl21::IsFramebufferEXT(r) }
    unsafe fn IsRenderbufferOES(&mut self, r: GLuint) -> GLboolean { gl21::IsRenderbufferEXT(r) }
    unsafe fn BindFramebufferOES(&mut self, t: GLenum, f: GLuint) { gl21::BindFramebufferEXT(t, f) }
    unsafe fn BindRenderbufferOES(&mut self, t: GLenum, r: GLuint) { gl21::BindRenderbufferEXT(t, r) }
    unsafe fn RenderbufferStorageOES(&mut self, t: GLenum, i: GLenum, w: GLsizei, h: GLsizei) { gl21::RenderbufferStorageEXT(t, i, w, h) }
    unsafe fn FramebufferRenderbufferOES(&mut self, t: GLenum, a: GLenum, rt: GLenum, r: GLuint) { gl21::FramebufferRenderbufferEXT(t, a, rt, r) }
    unsafe fn FramebufferTexture2DOES(&mut self, t: GLenum, a: GLenum, tt: GLenum, tex: GLuint, l: i32) { gl21::FramebufferTexture2DEXT(t, a, tt, tex, l) }
    unsafe fn GetFramebufferAttachmentParameterivOES(&mut self, target: GLenum, attachment: GLenum, pname: GLenum, params: *mut GLint) { gl21::GetFramebufferAttachmentParameterivEXT(target, attachment, pname, params) }
    unsafe fn CheckFramebufferStatusOES(&mut self, t: GLenum) -> GLenum { gl21::CheckFramebufferStatusEXT(t) }
    unsafe fn DeleteFramebuffersOES(&mut self, n: GLsizei, f: *const GLuint) { gl21::DeleteFramebuffersEXT(n, f) }
    unsafe fn DeleteRenderbuffersOES(&mut self, n: GLsizei, r: *const GLuint) { gl21::DeleteRenderbuffersEXT(n, r) }
    unsafe fn GenerateMipmapOES(&mut self, t: GLenum) { gl21::GenerateMipmapEXT(t) }
    unsafe fn GetBufferParameteriv(&mut self, t: GLenum, pn: GLenum, ps: *mut GLint) { gl21::GetBufferParameteriv(t, pn, ps) }
    unsafe fn GetRenderbufferParameterivOES(&mut self, t: GLenum, pn: GLenum, ps: *mut GLint) { gl21::GetRenderbufferParameterivEXT(t, pn, ps) }
    unsafe fn MapBufferOES(&mut self, target: GLenum, access: GLenum) -> *mut GLvoid { gl21::MapBuffer(target, access) }
    unsafe fn UnmapBufferOES(&mut self, target: GLenum) -> GLboolean { gl21::UnmapBuffer(target) }
}
