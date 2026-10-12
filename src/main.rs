use diorama_raytracer::camera::Camera;
use diorama_raytracer::render::{render_q, upscale_bilinear, Quality};
use diorama_raytracer::scene::build_diorama;
use diorama_raytracer::sky::Environment;
use diorama_raytracer::vec3::Vec3;
use raylib::prelude::*;
use std::time::Instant;

// Resolucion a la que se traza el raytracing (en la CPU). Mientras mas alta, mas nitido pero mas lento.
// Si su computadora es mas limitada, bajenla (ej. 380x260).
const RENDER_W: usize = 480;
const RENDER_H: usize = 320;

// Tamano de la ventana. La imagen se agranda en la CPU con interpolacion bilinear hasta este
// tamano, y raylib solo la dibuja 1:1 (sin que la tarjeta de video escale nada).
const WINDOW_W: usize = 960;
const WINDOW_H: usize = 640;

// Refinamiento progresivo cuando la camara se queda quieta, etapa por etapa: (ancho, alto, rayos por pixel).
//  1) imagen rapida, 2) con anti-aliasing, 3) etapa FINAL a la resolucion completa de la ventana
//  (sin agrandar nada: cada pixel de la pantalla se calcula con raytracing, 4 rayos por pixel).
// Si su computadora es lenta y la etapa 3 tarda demasiado, borren esa linea.
const REFINE_STAGES: [(usize, usize, u32); 3] = [
    (RENDER_W, RENDER_H, 1),
    (RENDER_W, RENDER_H, 4),
    (WINDOW_W, WINDOW_H, 4),
];

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
        .size(WINDOW_W as i32, WINDOW_H as i32)
        .title("Diorama Raytracer - Proyecto de Graficas por Computadora")
        .build();
    rl.set_target_fps(60);

    // Textura donde volcamos el resultado del raytracing (calculado en la CPU) para mostrarlo.
    // La tarjeta de video solo se usa para DIBUJAR esta textura; todo el calculo de cada pixel
    // (incluido el escalado bilinear) se hace en la CPU.
    let image = Image::gen_image_color(WINDOW_W as i32, WINDOW_H as i32, Color::BLACK);
    let mut texture = rl
        .load_texture_from_image(&thread, &image)
        .expect("no se pudo crear la textura");

    let rot_speed = 1.3_f32;
    let zoom_speed = 9.0_f32;

    // Preview adaptativo: mientras algo se mueve, se traza a 1/preview_div de la resolucion y
    // este divisor se ajusta solo segun lo rapido que sea su computadora (para que se mueva fluido).
    let mut preview_div: usize = 2;
    let mut stage: usize = 0; // siguiente etapa de refinamiento a dibujar (REFINE_STAGES.len() = terminado)
    let mut first = true;
    let mut last_ms = 0.0_f32;
    let mut last_label = String::from("-");

    while !rl.window_should_close() {
        let dt = rl.get_frame_time().min(0.05);
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
        let anim_time = rl.get_time() as f32;
        if dirty {
            // Algo cambio: preview rapido (baja resolucion, 1 rayo/pixel) escalado con bilinear.
            let env = Environment::new(time_of_day, anim_time);
            let t0 = Instant::now();
            let (pw, ph) = (RENDER_W / preview_div, RENDER_H / preview_div);
            let small = render_q(&scene, &cam, &env, pw, ph, 1, &Quality::PREVIEW);
            let pixels = upscale_bilinear(&small, pw, ph, WINDOW_W, WINDOW_H);
            last_ms = t0.elapsed().as_secs_f32() * 1000.0;
            last_label = format!("preview {}x{}", pw, ph);
            texture.update_texture(&pixels).expect("no se pudo actualizar la textura");
            stage = 0;

            // Ajuste automatico: si tarda mucho, bajamos la resolucion del preview; si sobra, la subimos.
            if last_ms > 40.0 && preview_div < 3 {
                preview_div += 1;
            } else if last_ms < 18.0 && preview_div > 1 {
                preview_div -= 1;
            }
        } else if stage < REFINE_STAGES.len() {
            // Todo quieto: refinamos la imagen etapa por etapa (cada una con mas rayos por pixel).
            let (rw, rh, spp) = REFINE_STAGES[stage];
            let env = Environment::new(time_of_day, anim_time);
            let t0 = Instant::now();
            let img = render_q(&scene, &cam, &env, rw, rh, spp, &Quality::FULL);
            // si la etapa ya es del tamano de la ventana no hay que agrandar nada
            let pixels = if rw == WINDOW_W && rh == WINDOW_H {
                img
            } else {
                upscale_bilinear(&img, rw, rh, WINDOW_W, WINDOW_H)
            };
            last_ms = t0.elapsed().as_secs_f32() * 1000.0;
            last_label = format!("{}x{} con {} rayos/pixel", rw, rh, spp);
            texture.update_texture(&pixels).expect("no se pudo actualizar la textura");
            stage += 1;
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

        let h = WINDOW_H as i32;
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        d.draw_texture(&texture, 0, 0, Color::WHITE);
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