use diorama_raytracer::camera::Camera;
use diorama_raytracer::scene::build_diorama;
use diorama_raytracer::vec3::Vec3;
use raylib::prelude::*;
use std::sync::Arc;

// Resolucion a la que se traza el raytracing. Mientras mas alta, mas nitido pero mas lento
// cada vez que la camara se mueve. Bajenla si su computadora es mas limitada, o subanla
// para tomar una captura final mas bonita (tecla P exporta una captura de la ventana).
const RENDER_W: usize = 380;
const RENDER_H: usize = 260;
const WINDOW_SCALE: i32 = 2;

fn main() {
    let scene = Arc::new(build_diorama());

    let mut cam = Camera {
        target: Vec3::new(8.0, 2.5, 8.0),
        yaw: 0.9,
        pitch: 0.5,
        distance: 22.0,
        fov: 0.85,
    };

    let (mut rl, thread) = raylib::init()
        .size(RENDER_W as i32 * WINDOW_SCALE, RENDER_H as i32 * WINDOW_SCALE)
        .title("Diorama Raytracer - Proyecto de Graficas por Computadora")
        .build();
    rl.set_target_fps(60);

    // Textura donde vamos a volcar el resultado del raytracing (CPU) para mostrarlo en pantalla.
    // Ojo: la tarjeta de video solo se usa para DIBUJAR esta textura en la ventana, todo el
    // calculo de color de cada pixel (el raytracing en si) se hace en la CPU en render::render().
    let image = Image::gen_image_color(RENDER_W as i32, RENDER_H as i32, Color::BLACK);
    let mut texture = rl
        .load_texture_from_image(&thread, &image)
        .expect("no se pudo crear la textura");

    let mut pixels = diorama_raytracer::render::render(Arc::clone(&scene), cam, RENDER_W, RENDER_H);
    texture
        .update_texture(&pixels)
        .expect("no se pudo actualizar la textura");

    let rot_speed = 1.3_f32;
    let zoom_speed = 9.0_f32;

    while !rl.window_should_close() {
        let dt = rl.get_frame_time();
        let mut dirty = false;

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
            cam.pitch = (cam.pitch - rot_speed * dt).max(-0.15);
            dirty = true;
        }
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

        if dirty {
            pixels = diorama_raytracer::render::render(Arc::clone(&scene), cam, RENDER_W, RENDER_H);
            texture
                .update_texture(&pixels)
                .expect("no se pudo actualizar la textura");
        }

        if rl.is_key_pressed(KeyboardKey::KEY_P) {
            // Guarda tal cual se ve la ventana en este momento (util para el video del README).
            rl.take_screenshot(&thread, "captura_diorama.png");
        }

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        d.draw_texture_ex(&texture, Vector2::new(0.0, 0.0), 0.0, WINDOW_SCALE as f32, Color::WHITE);
        d.draw_text(
            "Flechas: rotar camara  |  W/S o rueda del mouse: acercar/alejar  |  P: guardar captura",
            10,
            RENDER_H as i32 * WINDOW_SCALE - 26,
            18,
            Color::YELLOW,
        );
        d.draw_fps(RENDER_W as i32 * WINDOW_SCALE - 90, 10);
    }
}
