//! Headless offscreen OpenGL/CGL + `femtovg` + `vizia` PNG preview renderer for automated
//! visual verification of individual widgets, FX Unit cards, and full plugin UIs.

use std::path::Path;

use glow::HasContext;
use nih_plug_vizia::vizia::backend::{BackendContext, WindowDescription};
use nih_plug_vizia::vizia::image::RgbaImage;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

/// Renders a Vizia UI tree offscreen into a PNG file at `out_path` without opening any OS window.
///
/// `logical_w` and `logical_h` are in logical pixels; `scale_factor` multiplies them to produce
/// physical pixel dimensions (e.g., `1.0` for 100%, `1.5` for 150%).
pub fn render_ui_to_png<F>(
    logical_w: u32,
    logical_h: u32,
    scale_factor: f64,
    out_path: &Path,
    build_ui: F,
) -> Result<(), String>
where
    F: FnOnce(&mut Context),
{
    let scale = scale_factor.clamp(0.5, 4.0);
    let phys_w = ((logical_w as f64 * scale).round() as u32).max(1);
    let phys_h = ((logical_h as f64 * scale).round() as u32).max(1);

    let pixels = render_ui_to_rgba(logical_w, logical_h, scale, phys_w, phys_h, build_ui)?;
    if let Some(parent) = out_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create preview output dir: {e}"))?;
    }
    let img = RgbaImage::from_raw(phys_w, phys_h, pixels)
        .ok_or_else(|| "Failed to construct RGBA image from rendered buffer".to_string())?;
    img.save(out_path)
        .map_err(|e| format!("Failed to save PNG '{}': {e}", out_path.display()))
}

#[cfg(target_os = "macos")]
mod macos_gl {
    use std::ffi::{c_void, CStr};
    use std::ptr;

    use cgl::{
        kCGLNoError, kCGLPFAAccelerated, kCGLPFAAllowOfflineRenderers, kCGLPFAAlphaSize,
        kCGLPFAColorSize, kCGLPFAOpenGLProfile, CGLChoosePixelFormat, CGLContextObj,
        CGLCreateContext, CGLDestroyContext, CGLDestroyPixelFormat, CGLPixelFormatObj,
        CGLSetCurrentContext,
    };

    const CGL_OPENGL_3_2_CORE: i32 = 0x3200;

    pub struct OffscreenGlContext {
        ctx: CGLContextObj,
        gl_lib: *mut c_void,
    }

    impl OffscreenGlContext {
        pub fn new() -> Result<Self, String> {
            unsafe {
                let gl_lib = libc::dlopen(
                    c"/System/Library/Frameworks/OpenGL.framework/OpenGL".as_ptr(),
                    libc::RTLD_LAZY,
                );
                if gl_lib.is_null() {
                    return Err("Failed to load macOS OpenGL.framework".to_string());
                }

                let accel_attrs: [i32; 9] = [
                    kCGLPFAOpenGLProfile,
                    CGL_OPENGL_3_2_CORE,
                    kCGLPFAAccelerated,
                    kCGLPFAAllowOfflineRenderers,
                    kCGLPFAColorSize,
                    24,
                    kCGLPFAAlphaSize,
                    8,
                    0,
                ];
                let sw_attrs: [i32; 7] = [
                    kCGLPFAOpenGLProfile,
                    CGL_OPENGL_3_2_CORE,
                    kCGLPFAColorSize,
                    24,
                    kCGLPFAAlphaSize,
                    8,
                    0,
                ];

                let mut pix: CGLPixelFormatObj = ptr::null_mut();
                let mut npix: i32 = 0;
                let mut err = CGLChoosePixelFormat(accel_attrs.as_ptr(), &mut pix, &mut npix);
                if err != kCGLNoError || pix.is_null() {
                    err = CGLChoosePixelFormat(sw_attrs.as_ptr(), &mut pix, &mut npix);
                }
                if err != kCGLNoError || pix.is_null() {
                    libc::dlclose(gl_lib);
                    return Err(format!("CGLChoosePixelFormat failed with error {err}"));
                }

                let mut ctx: CGLContextObj = ptr::null_mut();
                let create_err = CGLCreateContext(pix, ptr::null_mut(), &mut ctx);
                CGLDestroyPixelFormat(pix);
                if create_err != kCGLNoError || ctx.is_null() {
                    libc::dlclose(gl_lib);
                    return Err(format!("CGLCreateContext failed with error {create_err}"));
                }

                let cur_err = CGLSetCurrentContext(ctx);
                if cur_err != kCGLNoError {
                    CGLDestroyContext(ctx);
                    libc::dlclose(gl_lib);
                    return Err(format!("CGLSetCurrentContext failed with error {cur_err}"));
                }

                Ok(Self { ctx, gl_lib })
            }
        }

        pub fn get_proc_addr(&self, name: &str) -> *const c_void {
            let Ok(cstr) = std::ffi::CString::new(name) else {
                return ptr::null();
            };
            unsafe { libc::dlsym(self.gl_lib, cstr.as_ptr()) }
        }

        pub fn get_proc_addr_cstr(&self, name: &CStr) -> *const c_void {
            unsafe { libc::dlsym(self.gl_lib, name.as_ptr()) }
        }
    }

    impl Drop for OffscreenGlContext {
        fn drop(&mut self) {
            unsafe {
                let _ = CGLSetCurrentContext(ptr::null_mut());
                if !self.ctx.is_null() {
                    let _ = CGLDestroyContext(self.ctx);
                }
                if !self.gl_lib.is_null() {
                    let _ = libc::dlclose(self.gl_lib);
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn render_ui_to_rgba<F>(
    logical_w: u32,
    logical_h: u32,
    scale: f64,
    phys_w: u32,
    phys_h: u32,
    build_ui: F,
) -> Result<Vec<u8>, String>
where
    F: FnOnce(&mut Context),
{
    let gl_ctx = macos_gl::OffscreenGlContext::new()?;

    let gl = unsafe { glow::Context::from_loader_function(|s| gl_ctx.get_proc_addr(s)) };

    // Create offscreen RGBA8 + Depth24Stencil8 framebuffer target for femtovg
    let (fbo, color_tex, depth_stencil_rbo) = unsafe {
        let fbo = gl
            .create_framebuffer()
            .map_err(|e| format!("create_framebuffer failed: {e}"))?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));

        let tex = gl
            .create_texture()
            .map_err(|e| format!("create_texture failed: {e}"))?;
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            phys_w as i32,
            phys_h as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            None,
        );
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(tex),
            0,
        );

        let rbo = gl
            .create_renderbuffer()
            .map_err(|e| format!("create_renderbuffer failed: {e}"))?;
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(rbo));
        gl.renderbuffer_storage(
            glow::RENDERBUFFER,
            glow::DEPTH24_STENCIL8,
            phys_w as i32,
            phys_h as i32,
        );
        gl.framebuffer_renderbuffer(
            glow::FRAMEBUFFER,
            glow::DEPTH_STENCIL_ATTACHMENT,
            glow::RENDERBUFFER,
            Some(rbo),
        );

        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        if status != glow::FRAMEBUFFER_COMPLETE {
            gl.delete_renderbuffer(rbo);
            gl.delete_texture(tex);
            gl.delete_framebuffer(fbo);
            return Err(format!("Offscreen framebuffer incomplete: 0x{status:x}"));
        }

        (fbo, tex, rbo)
    };

    let mut renderer = unsafe {
        vg::renderer::OpenGl::new_from_function_cstr(|s| gl_ctx.get_proc_addr_cstr(s))
            .map_err(|e| format!("femtovg OpenGl init failed: {e:?}"))?
    };
    renderer.set_screen_target(Some(fbo));

    let canvas = vg::Canvas::new(renderer).map_err(|e| format!("femtovg Canvas init failed: {e:?}"))?;

    let win_size = WindowSize::new(logical_w, logical_h);
    let mut context = Context::new(win_size, 1.0);
    context.remove_user_themes();

    let win_desc = WindowDescription::new()
        .with_inner_size(logical_w, logical_h)
        .with_scale_factor(1.0);

    BackendContext::new(&mut context).add_main_window(&win_desc, canvas, scale as f32);

    build_ui(&mut context);
    let _ = EventContext::new(&mut context).reload_styles();

    {
        let mut bc = BackendContext::new_with_event_manager(&mut context);
        bc.set_scale_factor(scale);
        bc.set_window_size(phys_w as f32, phys_h as f32);

        // Settle reactive bindings, dynamic child views, styles, and layout over several passes.
        for _ in 0..4 {
            bc.process_events();
            bc.process_data_updates();
            bc.process_events();
            bc.process_style_updates();
            bc.process_animations();
            bc.process_visual_updates();
        }
        bc.draw();
    }

    let row_bytes = phys_w as usize * 4;
    let mut raw = vec![0u8; row_bytes * phys_h as usize];
    unsafe {
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.finish();
        gl.read_pixels(
            0,
            0,
            phys_w as i32,
            phys_h as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(&mut raw),
        );
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.delete_renderbuffer(depth_stencil_rbo);
        gl.delete_texture(color_tex);
        gl.delete_framebuffer(fbo);
    }

    // OpenGL read_pixels origin is bottom-left; flip rows to top-left for PNG.
    let mut flipped = vec![0u8; raw.len()];
    for y in 0..phys_h as usize {
        let src_row = (phys_h as usize - 1 - y) * row_bytes;
        let dst_row = y * row_bytes;
        flipped[dst_row..dst_row + row_bytes].copy_from_slice(&raw[src_row..src_row + row_bytes]);
    }

    Ok(flipped)
}

#[cfg(not(target_os = "macos"))]
fn render_ui_to_rgba<F>(
    _logical_w: u32,
    _logical_h: u32,
    _scale: f64,
    _phys_w: u32,
    _phys_h: u32,
    _build_ui: F,
) -> Result<Vec<u8>, String>
where
    F: FnOnce(&mut Context),
{
    Err("Headless OpenGL preview rendering is currently implemented for macOS CGL".to_string())
}
