use diorama_raytracer::camera::Camera;
use diorama_raytracer::scene::build_diorama;
use diorama_raytracer::vec3::Vec3;
use std::sync::Arc;

#[test]
fn renders_without_panicking_and_has_varied_colors() {
    let scene = Arc::new(build_diorama());
    let cam = Camera {
        target: Vec3::new(8.0, 2.0, 8.0),
        yaw: 0.9,
        pitch: 0.5,
        distance: 20.0,
        fov: 0.9,
    };
    let pixels = diorama_raytracer::render::render(scene, cam, 80, 60);
    assert_eq!(pixels.len(), 80 * 60 * 4);

    // No deberia haber NaNs/colores fuera de rango (ya estan en u8, asi que solo revisamos variedad).
    let mut distinct = std::collections::HashSet::new();
    for chunk in pixels.chunks(4) {
        distinct.insert((chunk[0], chunk[1], chunk[2]));
    }
    // Si todo fuera un solo color, algo estaria mal (p.ej. que todos los rayos fallen y caigan al skybox).
    assert!(distinct.len() > 20, "se esperaban varios colores distintos, hubo {}", distinct.len());
}

#[test]
fn camera_orbit_moves_eye() {
    let cam1 = Camera { target: Vec3::new(0.0, 0.0, 0.0), yaw: 0.0, pitch: 0.0, distance: 10.0, fov: 0.9 };
    let cam2 = Camera { target: Vec3::new(0.0, 0.0, 0.0), yaw: 1.0, pitch: 0.3, distance: 10.0, fov: 0.9 };
    let e1 = cam1.eye();
    let e2 = cam2.eye();
    assert!((e1.x - e2.x).abs() > 0.01 || (e1.y - e2.y).abs() > 0.01);
}
