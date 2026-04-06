/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Shared utilities.

use super::gles11_raw as gles11; // constants only
use super::gles11_raw::types::{GLenum, GLfixed, GLfloat, GLint, GLsizei};
use super::GLES;

/// Convert a fixed-point scalar to a floating-point scalar.
pub fn fixed_to_float(fixed: GLfixed) -> GLfloat {
    ((fixed as f64) / ((1 << 16) as f64)) as f32
}

/// Convert a fixed-point 4-by-4 matrix to floating-point.
pub unsafe fn matrix_fixed_to_float(m: *const GLfixed) -> [GLfloat; 16] {
    let mut matrix = [0f32; 16];
    for (i, cell) in matrix.iter_mut().enumerate() {
        *cell = fixed_to_float(m.add(i).read_unaligned());
    }
    matrix
}

/// Type of a parameter, used in [ParamTable].
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ParamType {
    Boolean,
    Float,
    FloatSpecial,
    Int,
}

pub struct ParamTable(pub &'static [(GLenum, ParamType, usize)]);

impl ParamTable {
    pub fn contains(&self, pname: GLenum) -> bool {
        self.0.iter().any(|&(name, _, _)| name == pname)
    }

    pub fn get_type_info(&self, pname: GLenum) -> (ParamType, usize) {
        for &(name, type_, count) in self.0.iter() {
            if name == pname {
                return (type_, count);
            }
        }
        panic!("Unhandled parameter name: {:#x}", pname);
    }

    pub unsafe fn setx<F, I>(&self, mut f: F, mut i: I, pname: GLenum, param: GLfixed)
    where
        F: FnMut(GLfloat),
        I: FnMut(GLint),
    {
        let (type_, count) = self.get_type_info(pname);
        assert_eq!(count, 1);
        match type_ {
            ParamType::Float | ParamType::FloatSpecial => f(fixed_to_float(param)),
            ParamType::Int => i(param),
            ParamType::Boolean => i(if param != 0 { 1 } else { 0 }),
        }
    }

    pub unsafe fn setxv<F, I>(&self, mut f: F, mut i: I, pname: GLenum, params: *const GLfixed)
    where
        F: FnMut(*const GLfloat),
        I: FnMut(*const GLint),
    {
        let (type_, count) = self.get_type_info(pname);
        match type_ {
            ParamType::Float | ParamType::FloatSpecial => {
                let mut float_params = Vec::with_capacity(count);
                for j in 0..count {
                    float_params.push(fixed_to_float(params.add(j).read_unaligned()));
                }
                f(float_params.as_ptr());
            }
            ParamType::Int | ParamType::Boolean => {
                i(params as *const GLint);
            }
        }
    }

    pub fn assert_component_count(&self, pname: GLenum, expected: usize) {
        let (_type, actual) = self.get_type_info(pname);
        assert_eq!(actual, expected);
    }

    pub fn assert_known_param(&self, pname: GLenum) {
        let _ = self.get_type_info(pname);
    }
}

pub struct PalettedTextureFormat {
    pub index_is_nibble: bool,
    pub palette_entry_format: GLenum,
    pub palette_entry_type: GLenum,
}

impl PalettedTextureFormat {
    pub fn get_info(internalformat: GLenum) -> Option<Self> {
        match internalformat {
            gles11::PALETTE4_RGB8_OES => Some(Self {
                index_is_nibble: true,
                palette_entry_format: gles11::RGB,
                palette_entry_type: gles11::UNSIGNED_BYTE,
            }),
            gles11::PALETTE4_RGBA8_OES => Some(Self {
                index_is_nibble: true,
                palette_entry_format: gles11::RGBA,
                palette_entry_type: gles11::UNSIGNED_BYTE,
            }),
            gles11::PALETTE4_R5_G6_B5_OES => Some(Self {
                index_is_nibble: true,
                palette_entry_format: gles11::RGB,
                palette_entry_type: gles11::UNSIGNED_SHORT_5_6_5,
            }),
            gles11::PALETTE4_RGBA4_OES => Some(Self {
                index_is_nibble: true,
                palette_entry_format: gles11::RGBA,
                palette_entry_type: gles11::UNSIGNED_SHORT_4_4_4_4,
            }),
            gles11::PALETTE4_RGB5_A1_OES => Some(Self {
                index_is_nibble: true,
                palette_entry_format: gles11::RGBA,
                palette_entry_type: gles11::UNSIGNED_SHORT_5_5_5_1,
            }),
            gles11::PALETTE8_RGB8_OES => Some(Self {
                index_is_nibble: false,
                palette_entry_format: gles11::RGB,
                palette_entry_type: gles11::UNSIGNED_BYTE,
            }),
            gles11::PALETTE8_RGBA8_OES => Some(Self {
                index_is_nibble: false,
                palette_entry_format: gles11::RGBA,
                palette_entry_type: gles11::UNSIGNED_BYTE,
            }),
            gles11::PALETTE8_R5_G6_B5_OES => Some(Self {
                index_is_nibble: false,
                palette_entry_format: gles11::RGB,
                palette_entry_type: gles11::UNSIGNED_SHORT_5_6_5,
            }),
            gles11::PALETTE8_RGBA4_OES => Some(Self {
                index_is_nibble: false,
                palette_entry_format: gles11::RGBA,
                palette_entry_type: gles11::UNSIGNED_SHORT_4_4_4_4,
            }),
            gles11::PALETTE8_RGB5_A1_OES => Some(Self {
                index_is_nibble: false,
                palette_entry_format: gles11::RGBA,
                palette_entry_type: gles11::UNSIGNED_SHORT_5_5_5_1,
            }),
            _ => None,
        }
    }
}

pub unsafe fn try_decode_pvrtc(
    _gles: &mut dyn GLES,
    _target: GLenum,
    _level: GLint,
    _internalformat: GLenum,
    _width: GLsizei,
    _height: GLsizei,
    _border: GLint,
    _data: &[u8],
) -> bool {
    false
}

// DEFINING SHADER CONSTANT FOR N.O.V.A. 3 FIX
pub const GET_PARAMS: ParamTable = ParamTable(&[
    (gles11::MAX_TEXTURE_SIZE, ParamType::Int, 1),
    (0x8df8, ParamType::Int, 1), // GL_NUM_SHADER_BINARY_FORMATS
    (0x8df9, ParamType::Int, 0), // GL_SHADER_BINARY_FORMATS (Tell N3 we support 0 binary formats)
]);
