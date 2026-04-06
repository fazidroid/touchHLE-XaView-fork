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
    /// `GLboolean`
    Boolean,
    /// `GLfloat`
    Float,
    /// `GLint`
    Int,
    /// Placeholder type for things like colors which are floating-point
    /// but don't have the usual conversion behavior to/from integers etc.
    FloatSpecial,
    /// Hack to achieve `#[non_exhaustive]`-like behavior within this crate
    _NonExhaustive,
}

/// Table of parameter names, component types and component counts.
pub struct ParamTable(pub &'static [(GLenum, ParamType, u8)]);

impl ParamTable {
    /// Look up the component type and count for a parameter.
    pub fn get_type_info(&self, pname: GLenum) -> (ParamType, u8) {
        match self.0.iter().find(|&&(pname2, _, _)| pname == pname2) {
            Some(&(_, type_, count)) => (type_, count),
            None => {
                // FIXED: Instead of panicking on unknown queries like 0xBC2 (Alpha Test Ref)
                // or ES 2.0 queries, we return a default Int type. This allows the query 
                // to pass safely to the native host GPU driver.
                (ParamType::Int, 1)
            }
        }
    }

    /// Assert that a parameter name is recognized.
    pub fn assert_known_param(&self, pname: GLenum) {
        self.get_type_info(pname);
    }

    pub fn contains(&self, pname: GLenum) -> bool {
        // Explicitly allow common GLES 1.1 states and ES 2.0 extensions to bypass the check
        if pname == 0x0BC2 || pname == 0x8df8 || pname == 0x8df9 { 
            return true; 
        }
        self.0.iter().any(|(pname2, _, _)| pname == *pname2)
    }

    /// Assert that a parameter name is recognized and has a particular component count.
    pub fn assert_component_count(&self, pname: GLenum, provided_count: u8) {
        let (_type, actual_count) = self.get_type_info(pname);
        if actual_count != provided_count && actual_count != 1 {
            panic!(
                "Parameter {pname:#x} has component count {actual_count}, {provided_count} given."
            );
        }
    }

    /// Implements a fixed-point scalar (`x`) setter.
    pub unsafe fn setx<FF, FI>(&self, setf: FF, seti: FI, pname: GLenum, param: GLfixed)
    where
        FF: FnOnce(GLfloat),
        FI: FnOnce(GLint),
    {
        let (type_, component_count) = self.get_type_info(pname);
        assert!(component_count == 1);
        match type_ {
            ParamType::Float | ParamType::FloatSpecial => setf(fixed_to_float(param)),
            _ => seti(param),
        }
    }

    /// Implements a fixed-point vector (`xv`) setter.
    pub unsafe fn setxv<FFV, FIV>(
        &self,
        setfv: FFV,
        setiv: FIV,
        pname: GLenum,
        params: *const GLfixed,
    ) where
        FFV: FnOnce(*const GLfloat),
        FIV: FnOnce(*const GLint),
    {
        let (type_, count) = self.get_type_info(pname);
        match type_ {
            ParamType::Float | ParamType::FloatSpecial => {
                let mut params_float = [0.0; 16];
                let params_float = &mut params_float[..usize::from(count)];
                for (i, param_float) in params_float.iter_mut().enumerate() {
                    *param_float = fixed_to_float(params.add(i).read())
                }
                setfv(params_float.as_ptr())
            }
            _ => setiv(params),
        }
    }
}

/// Helper for implementing `glCompressedTexImage2D` with PVRTC decoding.
#[allow(clippy::too_many_arguments)]
pub fn try_decode_pvrtc(
    gles: &mut dyn GLES,
    target: GLenum,
    level: GLint,
    internalformat: GLenum,
    width: GLsizei,
    height: GLsizei,
    border: GLint,
    pvrtc_data: &[u8],
) -> bool {
    let is_2bit = match internalformat {
        gles11::COMPRESSED_RGB_PVRTC_4BPPV1_IMG |
        gles11::COMPRESSED_RGBA_PVRTC_4BPPV1_IMG => false,
        gles11::COMPRESSED_RGB_PVRTC_2BPPV1_IMG |
        gles11::COMPRESSED_RGBA_PVRTC_2BPPV1_IMG => true,
        _ => return false,
    };
    assert!(border == 0);
    let pixels = crate::image::decode_pvrtc(
        pvrtc_data,
        is_2bit,
        width.try_into().unwrap(),
        height.try_into().unwrap(),
    );
    unsafe {
        gles.TexImage2D(
            target,
            level,
            gles11::RGBA as _,
            width,
            height,
            border,
            gles11::RGBA,
            gles11::UNSIGNED_BYTE,
            pixels.as_ptr() as *const _,
        )
    };
    true
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
