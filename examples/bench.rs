use diorama_raytracer::camera::Camera;
use diorama_raytracer::scene::build_diorama;
use diorama_raytracer::vec3::Vec3;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let scene = Arc::new(build_diorama());
    let cam = Camera { target: Vec3::new(8.0,2.0,8.0), yaw: 0.9, pitch: 0.5, distance: 20.0, fov: 0.9 };
    for (w,h) in [(360,240),(640,427),(1280,853)] {
        let start = Instant::now();
        let px = diorama_raytracer::render::render(Arc::clone(&scene), cam, w, h);
        let elapsed = start.elapsed();
        println!("{}x{} -> {:?} ({} bytes)", w, h, elapsed, px.len());
    }
}
