/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Utilities for presenting frames to the window using an abstract OpenGL ES
//! implementation.

use super::gles11_raw as gles11; // constants and types only
use super::GLES;
use crate::matrix::Matrix;
use std::time::{Duration, Instant};

pub struct FpsCounter {
    time: std::time::Instant,
    frames: u32,
}
impl FpsCounter {
    pub fn start() -> Self {
        FpsCounter {
            time: Instant::now(),
            frames: 0,
        }
    }

    pub fn count_frame(&mut self, label: std::fmt::Arguments<'_>) {
        self.frames += 1;
        let now = Instant::now();
        let duration = now - self.time;
        if duration >= Duration::from_secs(1) {
            self.time = now;
            echo!(
                "touchHLE: {} FPS: {:.2}",
                label,
                std::mem::take(&mut self.frames) as f32 / duration.as_secs_f32()
            );
        }
    }
}

/// Present the the latest frame (e.g. the app's splash screen or rendering
/// output), provided as a texture bound to `GL_TEXTURE_2D`, by drawing it on
/// the window. It may be rotated, scaled and/or letterboxed as necessary. The
/// virtual cursor is also drawn if it should be currently visible.
///
/// The provided context must be current.
// SafePresentFix
pub unsafe fn present_frame(
    gles: &mut dyn GLES,
    viewport: (u32, u32, u32, u32),
    rotation_matrix: Matrix<2>,
    virtual_cursor_visible_at: Option<(f32, f32, bool)>,
) {
    use gles11::types::*;
    let is_gles2 = gles.is_gles2();

    gles.Viewport(viewport.0 as _, viewport.1 as _, viewport.2 as _, viewport.3 as _);
    gles.ClearColor(0.0, 0.0, 0.0, 1.0);
    gles.Clear(gles11::COLOR_BUFFER_BIT | gles11::DEPTH_BUFFER_BIT | gles11::STENCIL_BUFFER_BIT);
    gles.BindBuffer(gles11::ARRAY_BUFFER, 0);

    let vertices: [f32; 12] = [
        -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0,
    ];
    let tex_coords: [f32; 12] = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let matrix = Matrix::<4>::from(&rotation_matrix);

    static mut ES2_PROG: GLuint = 0;
    static mut ES2_POS: GLint = 0;
    static mut ES2_TEX: GLint = 0;
    static mut ES2_MAT: GLint = 0;
    static mut ES2_COL: GLint = 0;
    static mut ES2_TEX_SAMPLER: GLint = 0;

    if is_gles2 {
        if ES2_PROG == 0 {
            let vs_src = "attribute vec4 position;\nattribute vec2 texCoord;\nuniform mat4 texMatrix;\nvarying vec2 v_texCoord;\nvoid main() {\n    gl_Position = position;\n    v_texCoord = (texMatrix * vec4(texCoord, 0.0, 1.0)).xy;\n}\0";
            let fs_src = "precision mediump float;\nvarying vec2 v_texCoord;\nuniform sampler2D tex;\nuniform vec4 color;\nvoid main() {\n    if (color.a > 0.0) {\n        gl_FragColor = color;\n    } else {\n        gl_FragColor = texture2D(tex, v_texCoord);\n    }\n}\0";
            let vs = gles.CreateShader(0x8B31);
            let vs_ptr = [vs_src.as_ptr() as *const std::ffi::c_char];
            let vs_len = [vs_src.len() as GLint - 1];
            gles.ShaderSource(vs, 1, vs_ptr.as_ptr(), vs_len.as_ptr());
            gles.CompileShader(vs);
            let fs = gles.CreateShader(0x8B30);
            let fs_ptr = [fs_src.as_ptr() as *const std::ffi::c_char];
            let fs_len = [fs_src.len() as GLint - 1];
            gles.ShaderSource(fs, 1, fs_ptr.as_ptr(), fs_len.as_ptr());
            gles.CompileShader(fs);
            ES2_PROG = gles.CreateProgram();
            gles.AttachShader(ES2_PROG, vs);
            gles.AttachShader(ES2_PROG, fs);
            gles.LinkProgram(ES2_PROG);
            ES2_POS = gles.GetAttribLocation(ES2_PROG, c"position".as_ptr() as *const _);
            ES2_TEX = gles.GetAttribLocation(ES2_PROG, c"texCoord".as_ptr() as *const _);
            ES2_MAT = gles.GetUniformLocation(ES2_PROG, c"texMatrix".as_ptr() as *const _);
            ES2_COL = gles.GetUniformLocation(ES2_PROG, c"color".as_ptr() as *const _);
            ES2_TEX_SAMPLER = gles.GetUniformLocation(ES2_PROG, c"tex".as_ptr() as *const _);
        }
        gles.UseProgram(ES2_PROG);
        gles.Uniform4f(ES2_COL, 0.0, 0.0, 0.0, 0.0);
        gles.UniformMatrix4fv(ES2_MAT, 1, 0, matrix.columns().as_ptr() as *const _);

        let mut active_tex: GLint = 0;
        gles.GetIntegerv(gles11::ACTIVE_TEXTURE, &mut active_tex);
        let tex_unit = active_tex - gles11::TEXTURE0 as GLint;
        if ES2_TEX_SAMPLER >= 0 {
            gles.Uniform1i(ES2_TEX_SAMPLER, tex_unit);
        }

        if ES2_POS >= 0 {
            gles.EnableVertexAttribArray(ES2_POS as GLuint);
            gles.VertexAttribPointer(ES2_POS as GLuint, 2, gles11::FLOAT, 0, 0, vertices.as_ptr() as *const _);
        }
        if ES2_TEX >= 0 {
            gles.EnableVertexAttribArray(ES2_TEX as GLuint);
            gles.VertexAttribPointer(ES2_TEX as GLuint, 2, gles11::FLOAT, 0, 0, tex_coords.as_ptr() as *const _);
        }
        gles.DrawArrays(gles11::TRIANGLES, 0, 6);
    } else {
        gles.EnableClientState(gles11::VERTEX_ARRAY);
        gles.VertexPointer(2, gles11::FLOAT, 0, vertices.as_ptr() as *const GLvoid);
        gles.EnableClientState(gles11::TEXTURE_COORD_ARRAY);
        gles.TexCoordPointer(2, gles11::FLOAT, 0, tex_coords.as_ptr() as *const GLvoid);
        gles.MatrixMode(gles11::TEXTURE);
        gles.LoadMatrixf(matrix.columns().as_ptr() as *const _);
        gles.Enable(gles11::TEXTURE_2D);
        gles.DrawArrays(gles11::TRIANGLES, 0, 6);
        gles.LoadIdentity();
    }

    if let Some((x, y, pressed)) = virtual_cursor_visible_at {
        let (vx, vy, vw, vh) = viewport;
        let x = x - vx as f32;
        let y = y - vy as f32;
        gles.Enable(gles11::BLEND);
        gles.BlendFunc(gles11::ONE, gles11::ONE_MINUS_SRC_ALPHA);
        let radius = 10.0;
        let mut cursor_vertices = vertices;
        for i in (0..cursor_vertices.len()).step_by(2) {
            cursor_vertices[i] = (cursor_vertices[i] * radius + x) / (vw as f32 / 2.0) - 1.0;
            cursor_vertices[i + 1] = 1.0 - (cursor_vertices[i + 1] * radius + y) / (vh as f32 / 2.0);
        }
        if is_gles2 {
            gles.Uniform4f(ES2_COL, 0.0, 0.0, 0.0, if pressed { 2.0 / 3.0 } else { 1.0 / 3.0 });
            if ES2_TEX >= 0 { gles.DisableVertexAttribArray(ES2_TEX as GLuint); }
            if ES2_POS >= 0 {
                gles.VertexAttribPointer(ES2_POS as GLuint, 2, gles11::FLOAT, 0, 0, cursor_vertices.as_ptr() as *const _);
            }
            gles.DrawArrays(gles11::TRIANGLES, 0, 6);
            if ES2_POS >= 0 { gles.DisableVertexAttribArray(ES2_POS as GLuint); }
            gles.UseProgram(0);
        } else {
            gles.DisableClientState(gles11::TEXTURE_COORD_ARRAY);
            gles.Disable(gles11::TEXTURE_2D);
            gles.Color4f(0.0, 0.0, 0.0, if pressed { 2.0 / 3.0 } else { 1.0 / 3.0 });
            gles.VertexPointer(2, gles11::FLOAT, 0, cursor_vertices.as_ptr() as *const GLvoid);
            gles.DrawArrays(gles11::TRIANGLES, 0, 6);
        }
    } else if is_gles2 {
        if ES2_POS >= 0 { gles.DisableVertexAttribArray(ES2_POS as GLuint); }
        if ES2_TEX >= 0 { gles.DisableVertexAttribArray(ES2_TEX as GLuint); }
        gles.UseProgram(0);
    }
}
