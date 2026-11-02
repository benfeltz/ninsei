//! The CRT shader: cool-retro-term-style glass over the whole window.
//!
//! At the end of each frame a paint callback (the top layer) copies what egui drew into a
//! texture, keeps a fading burn-in history of it, and redraws the window through one fragment
//! shader: curvature, bezel, bloom, scanlines, noise, flicker, jitter, RGB shift and glare.
//! Geometry matches `ninsei_core::display::screen_to_content`, which the mouse also follows.
//!
//! If the GL context can't run it (no GLSL 1.40 / ES 3.00), [`CrtGl::failed`] turns true and the
//! app falls back to the old scanline overlay.

use std::sync::{Arc, Mutex};

use eframe::egui_glow::{self, ShaderVersion};
use eframe::glow::{self, HasContext};
use egui::{Context, Id, LayerId, Order};
use ninsei_core::display::{Effects, Raster};

/// Seconds for burn-in to fade by half at `burn_in` 1.0.
const BURN_HALF_LIFE: f32 = 0.5;

pub struct CrtGl {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    gpu: Option<Gpu>,
    failed: bool,
}

struct Gpu {
    crt: glow::Program,
    burn: glow::Program,
    vao: glow::VertexArray,
    /// This frame, copied from the window.
    cur: glow::Texture,
    /// Burn-in history, ping-ponged.
    hist: [glow::Texture; 2],
    fbo: [glow::Framebuffer; 2],
    size: [u32; 2],
    flip: usize,
    last_time: f64,
}

impl CrtGl {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
        }
    }

    /// The shader couldn't start on this GL; use the fallback overlay.
    pub fn failed(&self) -> bool {
        self.state.lock().map(|s| s.failed).unwrap_or(true)
    }

    /// Queues the CRT pass as the last thing drawn this frame.
    pub fn paint(&self, ctx: &Context, effects: Effects) {
        let state = Arc::clone(&self.state);
        let time = ctx.input(|i| i.time);
        let callback = egui_glow::CallbackFn::new(move |info, painter| {
            let Ok(mut st) = state.lock() else { return };
            if st.failed {
                return;
            }
            let gl = painter.gl();
            if st.gpu.is_none() {
                // SAFETY: called on the GL thread with egui_glow's live context.
                match unsafe { Gpu::new(gl) } {
                    Ok(g) => st.gpu = Some(g),
                    Err(e) => {
                        log_failure(&e);
                        st.failed = true;
                        return;
                    }
                }
            }
            if let Some(g) = st.gpu.as_mut() {
                let [w, h] = info.screen_size_px;
                // SAFETY: as above; `draw` restores what egui_glow doesn't re-apply itself.
                unsafe { g.draw(gl, w, h, &effects, time) };
            }
        });
        let rect = ctx.content_rect();
        ctx.layer_painter(LayerId::new(Order::Debug, Id::new("crt-shader")))
            .add(egui::PaintCallback {
                rect,
                callback: Arc::new(callback),
            });
        ctx.request_repaint();
    }
}

fn log_failure(e: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("crt shader unavailable, using the overlay: {e}");
    #[cfg(target_arch = "wasm32")]
    let _ = e;
}

const VERTEX: &str = r#"
out vec2 v_uv;
void main() {
    // One triangle that covers the screen; no vertex buffer needed.
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    v_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

const BURN: &str = r#"
in vec2 v_uv;
out vec4 o_color;
uniform sampler2D u_cur;
uniform sampler2D u_prev;
uniform float u_decay;
void main() {
    vec3 c = texture(u_cur, v_uv).rgb;
    vec3 p = texture(u_prev, v_uv).rgb * u_decay;
    o_color = vec4(max(c, p), 1.0);
}
"#;

const CRT: &str = r#"
in vec2 v_uv;
out vec4 o_color;
uniform sampler2D u_img;
uniform vec2 u_res;
uniform float u_time;
uniform float u_frame;
uniform float u_curve;
uniform float u_radius;
uniform float u_bloom;
uniform float u_noise;
uniform float u_flicker;
uniform float u_hsync;
uniform float u_jitter;
uniform float u_rgb;
uniform float u_glow;
uniform float u_bright;
uniform float u_contrast;
uniform float u_ambient;
uniform float u_shine;
uniform float u_raster;
uniform vec3 u_frame_color;

float hash(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }
vec3 tap(vec2 c) { return texture(u_img, vec2(c.x, 1.0 - c.y)).rgb; }

void main() {
    // y down, like the game and display::screen_to_content.
    vec2 uv = vec2(v_uv.x, 1.0 - v_uv.y);
    vec2 inner = (uv - u_frame) / (1.0 - 2.0 * u_frame);
    vec2 q = abs(inner - 0.5) - (0.5 - u_radius);
    float tube = length(max(q, 0.0)) - u_radius;
    vec2 cc = inner - 0.5;
    vec2 c = 0.5 + cc * (1.0 + dot(cc, cc) * 4.0 * u_curve);
    bool on_glass = tube <= 0.0;
    bool picture = on_glass && c.x >= 0.0 && c.x <= 1.0 && c.y >= 0.0 && c.y <= 1.0;
    if (!picture) {
        vec3 col = vec3(0.0);
        if (!on_glass) {
            // The bezel: lit from above, with a shine along the glass edge.
            float lit = 0.25 + 0.2 * (1.0 - uv.y);
            float edge = exp(-tube * 40.0);
            col = u_frame_color * lit * (0.6 + 0.8 * u_shine * edge);
        }
        o_color = vec4(col, 1.0);
        return;
    }
    float line = floor(c.y * u_res.y);
    float tick = floor(u_time * 24.0);
    c.x += (hash(vec2(line, tick)) - 0.5) * u_jitter * 0.0012;
    c.x += sin(c.y * 20.0 + u_time * 3.0) * u_hsync * 0.0015;

    float d = u_rgb * 0.003;
    vec3 col = vec3(tap(c + vec2(d, 0.0)).r, tap(c).g, tap(c - vec2(d, 0.0)).b);

    vec3 halo = vec3(0.0);
    for (int i = 0; i < 8; i++) {
        float a = float(i) * 0.785398;
        vec2 dir = vec2(cos(a), sin(a)) / u_res;
        halo += tap(c + dir * 2.5) + tap(c + dir * 6.0) * 0.6;
    }
    col += halo / 12.8 * u_bloom * 0.9;

    if (u_raster > 1.5) {
        vec2 g = 0.5 + 0.5 * cos(uv * u_res * 3.14159);
        col *= 0.72 + 0.28 * g.x * g.y;
    } else if (u_raster > 0.5) {
        float s = 0.5 + 0.5 * cos(uv.y * u_res.y * 3.14159);
        col *= 0.62 + 0.38 * s;
    }

    float band = exp(-pow((fract(c.y - u_time * 0.08) - 0.5) * 10.0, 2.0));
    col *= 1.0 + band * u_glow * 0.35;
    col += (hash(c * u_res + fract(u_time) * 100.0) - 0.5) * u_noise * 0.18;
    col *= 1.0 - u_flicker * 0.12 * hash(vec2(tick, 7.0));

    col = (col - 0.5) * (0.5 + u_contrast * 0.625) + 0.5;
    col *= u_bright * 2.0;

    float vig = pow(max(16.0 * c.x * c.y * (1.0 - c.x) * (1.0 - c.y), 0.0), 0.12);
    col *= mix(1.0, vig, 0.7);
    col += vec3(u_ambient * 0.07 * (1.0 - smoothstep(0.0, 0.8, length(inner - vec2(0.25, 0.2)))));
    o_color = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

impl Gpu {
    unsafe fn new(gl: &glow::Context) -> Result<Self, String> {
        let version = ShaderVersion::get(gl);
        if !version.is_new_shader_interface() {
            return Err(format!("needs GLSL 1.40 / ES 3.00, have {version:?}"));
        }
        let mut header = version.version_declaration().to_string();
        if matches!(version, ShaderVersion::Es300) {
            header.push_str("precision highp float;\n");
        }
        unsafe {
            let crt = program(gl, &header, CRT)?;
            let burn = program(gl, &header, BURN)?;
            let vao = gl.create_vertex_array()?;
            let cur = gl.create_texture()?;
            let hist = [gl.create_texture()?, gl.create_texture()?];
            let fbo = [gl.create_framebuffer()?, gl.create_framebuffer()?];
            Ok(Self {
                crt,
                burn,
                vao,
                cur,
                hist,
                fbo,
                size: [0, 0],
                flip: 0,
                last_time: 0.0,
            })
        }
    }

    unsafe fn resize(&mut self, gl: &glow::Context, w: u32, h: u32) {
        unsafe {
            for (i, tex) in [self.cur, self.hist[0], self.hist[1]]
                .into_iter()
                .enumerate()
            {
                gl.bind_texture(glow::TEXTURE_2D, Some(tex));
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA8 as i32,
                    w as i32,
                    h as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
                for (p, v) in [
                    (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                    (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                    (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                    (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
                ] {
                    gl.tex_parameter_i32(glow::TEXTURE_2D, p, v as i32);
                }
                if i > 0 {
                    let fbo = self.fbo[i - 1];
                    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
                    gl.framebuffer_texture_2d(
                        glow::FRAMEBUFFER,
                        glow::COLOR_ATTACHMENT0,
                        glow::TEXTURE_2D,
                        Some(tex),
                        0,
                    );
                    gl.clear_color(0.0, 0.0, 0.0, 1.0);
                    gl.clear(glow::COLOR_BUFFER_BIT);
                }
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }
        self.size = [w, h];
    }

    unsafe fn draw(&mut self, gl: &glow::Context, w: u32, h: u32, e: &Effects, time: f64) {
        if w == 0 || h == 0 {
            return;
        }
        let dt = (time - self.last_time).clamp(0.0, 0.1) as f32;
        self.last_time = time;
        unsafe {
            if self.size != [w, h] {
                self.resize(gl, w, h);
            }
            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::BLEND);
            gl.bind_vertex_array(Some(self.vao));

            // 1. Grab what egui drew this frame.
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.cur));
            gl.copy_tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, 0, 0, 0, w as i32, h as i32);

            // 2. Burn-in: the brighter of now and a fading memory of before.
            let (prev, next) = (self.flip, 1 - self.flip);
            let decay = if e.burn_in <= 0.0 {
                0.0
            } else {
                0.5f32.powf(dt / (e.burn_in * BURN_HALF_LIFE))
            };
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo[next]));
            gl.viewport(0, 0, w as i32, h as i32);
            gl.use_program(Some(self.burn));
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.hist[prev]));
            set_i(gl, self.burn, "u_cur", 0);
            set_i(gl, self.burn, "u_prev", 1);
            set_f(gl, self.burn, "u_decay", decay);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);

            // 3. The glass: draw the result to the window.
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.viewport(0, 0, w as i32, h as i32);
            gl.use_program(Some(self.crt));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.hist[next]));
            let p = self.crt;
            set_i(gl, p, "u_img", 0);
            if let Some(l) = gl.get_uniform_location(p, "u_res") {
                gl.uniform_2_f32(Some(&l), w as f32, h as f32);
            }
            if let Some(l) = gl.get_uniform_location(p, "u_frame_color") {
                let [r, g, b] = e.frame_color;
                gl.uniform_3_f32(Some(&l), r, g, b);
            }
            let raster = match e.rasterization {
                Raster::None => 0.0,
                Raster::Scanline => 1.0,
                Raster::Pixel => 2.0,
            };
            for (name, v) in [
                ("u_time", (time % 1000.0) as f32),
                ("u_frame", e.frame()),
                ("u_curve", e.curve()),
                ("u_radius", e.radius()),
                ("u_bloom", e.bloom),
                ("u_noise", e.static_noise),
                ("u_flicker", e.flickering),
                ("u_hsync", e.horizontal_sync),
                ("u_jitter", e.jitter),
                ("u_rgb", e.rgb_shift),
                ("u_glow", e.glowing_line),
                ("u_bright", e.brightness),
                ("u_contrast", e.contrast),
                ("u_ambient", e.ambient_light),
                ("u_shine", e.frame_shininess),
                ("u_raster", raster),
            ] {
                set_f(gl, p, name, v);
            }
            gl.draw_arrays(glow::TRIANGLES, 0, 3);

            gl.bind_vertex_array(None);
            gl.active_texture(glow::TEXTURE0);
        }
        self.flip = next_of(self.flip);
    }
}

fn next_of(i: usize) -> usize {
    1 - i
}

unsafe fn set_f(gl: &glow::Context, p: glow::Program, name: &str, v: f32) {
    unsafe {
        if let Some(l) = gl.get_uniform_location(p, name) {
            gl.uniform_1_f32(Some(&l), v);
        }
    }
}

unsafe fn set_i(gl: &glow::Context, p: glow::Program, name: &str, v: i32) {
    unsafe {
        if let Some(l) = gl.get_uniform_location(p, name) {
            gl.uniform_1_i32(Some(&l), v);
        }
    }
}

unsafe fn program(
    gl: &glow::Context,
    header: &str,
    fragment: &str,
) -> Result<glow::Program, String> {
    unsafe {
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        for (kind, body) in [
            (glow::VERTEX_SHADER, VERTEX),
            (glow::FRAGMENT_SHADER, fragment),
        ] {
            let s = gl.create_shader(kind)?;
            gl.shader_source(s, &format!("{header}{body}"));
            gl.compile_shader(s);
            if !gl.get_shader_compile_status(s) {
                return Err(gl.get_shader_info_log(s));
            }
            gl.attach_shader(program, s);
            shaders.push(s);
        }
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            return Err(gl.get_program_info_log(program));
        }
        for s in shaders {
            gl.detach_shader(program, s);
            gl.delete_shader(s);
        }
        Ok(program)
    }
}
