use diorama_raytracer::camera::Camera;
use diorama_raytracer::render::{render, upscale_nearest};
use diorama_raytracer::scene::build_diorama;
use diorama_raytracer::sky::Environment;
use diorama_raytracer::vec3::Vec3;
use raylib::prelude::*;
use std::time::Instant;

// Resolucion a la que se traza el raytracing. Mientras mas alta, mas nitido pero mas lento.
// Bajenla si su computadora es mas limitada.
const RENDER_W: usize = 380;
const RENDER_H: usize = 260;
const WINDOW_SCALE: i32 = 2;

// Calidad completa (cuando la camara esta quieta) y calidad "preview" (mientras se mueve).
const FULL_SAMPLES: u32 = 4; // rayos por pixel (anti-aliasing)
const PREVIEW_DIV: usize = 2; // el preview se traza a 1/2 de resolucion, 1 rayo por pixel

fn main() {
    let scene = build_diorama();

    let mut cam = Camera {
        target: Vec3::new(8.0, 3.0, 8.0),
        yaw: 0.9,
        pitch: 0.40,
        distance: 23.0,
        fov: 0.9,
    };
    let mut time_of_day = 0.20_f32; // 0 = amanecer, 0.25 = mediodia, 0.5 = atardecer, 0.75 = medianoche
    let mut auto_orbit = false;
    let mut auto_day = false;

    let (mut rl, thread) = raylib::init()
        .size(RENDER_W as i32 * WINDOW_SCALE, RENDER_H as i32 * WINDOW_SCALE)
        .title("Diorama Raytracer - Proyecto de Graficas por Computadora")
        .build();
    rl.set_target_fps(60);

    // Textura donde volcamos el resultado del raytracing (calculado en la CPU) para mostrarlo.
    // La tarjeta de video solo se usa para DIBUJAR esta textura; todo el calculo de cada pixel
    // se hace en la CPU en render::render().
    let image = Image::gen_image_color(RENDER_W as i32, RENDER_H as i32, Color::BLACK);
    let mut texture = rl
        .load_texture_from_image(&thread, &image)
        .expect("no se pudo crear la textura");

    let rot_speed = 1.3_f32;
    let zoom_speed = 9.0_f32;

    let mut need_full = true; // hay que hacer un render de calidad completa
    let mut first = true;
    let mut last_ms = 0.0_f32;
    let mut last_label = "completo";

    while !rl.window_should_close() {
        let dt = rl.get_frame_time();
        let mut dirty = first;
        first = false;

        // --- Rotar camara ---
        if rl.is_key_down(KeyboardKey::KEY_LEFT) {
            cam.yaw -= rot_speed * dt;
            dirty = true;
        }
        if rl.is_key_down(KeyboardKey::KEY_RIGHT) {
            cam.yaw += rot_speed * dt;
            dirty = true;
        }
        if rl.is_key_down(KeyboardKey::KEY_UP) {
            cam.pitch = (cam.pitch + rot_speed * dt).min(1.45);
            dirty = true;
        }
        if rl.is_key_down(KeyboardKey::KEY_DOWN) {
            cam.pitch = (cam.pitch - rot_speed * dt).max(-0.05);
            dirty = true;
        }
        // Arrastrar con el mouse (boton izquierdo)
        if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            let d = rl.get_mouse_delta();
            if d.x != 0.0 || d.y != 0.0 {
                cam.yaw -= d.x * 0.008;
                cam.pitch = (cam.pitch + d.y * 0.008).clamp(-0.05, 1.45);
                dirty = true;
            }
        }
        // --- Acercar / alejar ---
        if rl.is_key_down(KeyboardKey::KEY_W) {
            cam.distance = (cam.distance - zoom_speed * dt).max(4.0);
            dirty = true;
        }
        if rl.is_key_down(KeyboardKey::KEY_S) {
            cam.distance = (cam.distance + zoom_speed * dt).min(45.0);
            dirty = true;
        }
        let wheel = rl.get_mouse_wheel_move();
        if wheel.abs() > 0.01 {
            cam.distance = (cam.distance - wheel * 1.5).clamp(4.0, 45.0);
            dirty = true;
        }

        // --- Automatico: orbita y ciclo dia/noche ---
        if rl.is_key_pressed(KeyboardKey::KEY_R) {
            auto_orbit = !auto_orbit;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_T) {
            auto_day = !auto_day;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_N) {
            // salto rapido entre dia y noche
            time_of_day = if Environment::new(time_of_day, 0.0).day_amt > 0.5 { 0.75 } else { 0.20 };
            dirty = true;
        }
        if auto_orbit {
            cam.yaw += 0.45 * dt;
            dirty = true;
        }
        if auto_day {
            time_of_day = (time_of_day + dt / 40.0) % 1.0;
            dirty = true;
        }

        // --- Render ---
        // Mientras algo cambia: preview rapido (1/2 resolucion, 1 rayo/pixel).
        // Cuando todo se queda quieto: un render de calidad completa (4 rayos/pixel).
        let anim_time = rl.get_time() as f32;
        if dirty {
            let env = Environment::new(time_of_day, anim_time);
            let t0 = Instant::now();
            let (pw, ph) = (RENDER_W / PREVIEW_DIV, RENDER_H / PREVIEW_DIV);
            let small = render(&scene, &cam, &env, pw, ph, 1);
            let pixels = upscale_nearest(&small, pw, ph, RENDER_W, RENDER_H);
            last_ms = t0.elapsed().as_secs_f32() * 1000.0;
            last_label = "preview";
            texture.update_texture(&pixels).expect("no se pudo actualizar la textura");
            need_full = true;
        } else if need_full {
            let env = Environment::new(time_of_day, anim_time);
            let t0 = Instant::now();
            let pixels = render(&scene, &cam, &env, RENDER_W, RENDER_H, FULL_SAMPLES);
            last_ms = t0.elapsed().as_secs_f32() * 1000.0;
            last_label = "completo";
            texture.update_texture(&pixels).expect("no se pudo actualizar la textura");
            need_full = false;
        }

        if rl.is_key_pressed(KeyboardKey::KEY_P) {
            // Guarda tal cual se ve la ventana en este momento (util para el README).
            rl.take_screenshot(&thread, "captura_diorama.png");
        }

        let status = format!(
            "render {}: {:.0} ms | hora {:.2} | orbita {} | ciclo dia/noche {}",
            last_label,
            last_ms,
            time_of_day,
            if auto_orbit { "ON" } else { "off" },
            if auto_day { "ON" } else { "off" },
        );

        let h = RENDER_H as i32 * WINDOW_SCALE;
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        d.draw_texture_ex(&texture, Vector2::new(0.0, 0.0), 0.0, WINDOW_SCALE as f32, Color::WHITE);
        d.draw_text(
            "Flechas/arrastrar: rotar | W/S/rueda: zoom | R: orbita | T: ciclo dia/noche | N: dia<->noche | P: captura",
            10,
            h - 46,
            16,
            Color::YELLOW,
        );
        d.draw_text(&status, 10, h - 24, 16, Color::YELLOW);
    }
}