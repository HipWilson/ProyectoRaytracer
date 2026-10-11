// Mide cuanto tarda el render a distintas resoluciones y rayos por pixel en SU maquina:
//   cargo run --release --example bench
use diorama_raytracer::camera::Camera;
use diorama_raytracer::render::render;
use diorama_raytracer::scene::build_diorama;
use diorama_raytracer::sky::Environment;
use diorama_raytracer::vec3::Vec3;
use std::time::Instant;

fn main() {
    let t0 = Instant::now();
    let scene = build_diorama();
    println!("construir escena + skybox: {:?}", t0.elapsed());
    let cam = Camera { target: Vec3::new(8.0, 3.0, 8.0), yaw: 0.9, pitch: 0.4, distance: 23.0, fov: 0.9 };
    let env = Environment::noon();
    for &(w, h, spp) in &[(190, 130, 1), (380, 260, 1), (380, 260, 4), (640, 427, 4), (960, 640, 8)] {
        let start = Instant::now();
        let px = render(&scene, &cam, &env, w, h, spp);
        println!("{}x{} @ {} rayos/pixel -> {:?} ({} bytes)", w, h, spp, start.elapsed(), px.len());
    }
}