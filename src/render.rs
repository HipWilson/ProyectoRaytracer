use std::sync::Arc;
use std::thread;

use crate::camera::Camera;
use crate::ray::Ray;
use crate::scene::Scene;
use crate::texture::face_tangents;
use crate::vec3::{Color, Vec3};

const MAX_DEPTH: u32 = 4;
const LIGHT_DIR: Vec3 = Vec3 { x: 0.45, y: 0.85, z: 0.35 };
const SAMPLES_PER_PIXEL: u32 = 4; // anti-aliasing: bajar a 1-2 si el PC es lento

// xorshift32, para no depender de ninguna libreria externa de numeros aleatorios
struct Rng(u32);
impl Rng {
    fn new(seed: u32) -> Self {
        Rng(if seed == 0 { 0x9E3779B9 } else { seed })
    }
    fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / 16_777_216.0
    }
}

// Fresnel/Schlick: mezcla reflexion/refraccion segun el angulo de vision
fn schlick(cosine: f32, ref_idx: f32) -> f32 {
    let r0 = ((1.0 - ref_idx) / (1.0 + ref_idx)).powi(2);
    r0 + (1.0 - r0) * (1.0 - cosine).powi(5)
}

fn ray_color(scene: &Scene, ray: &Ray, depth: u32) -> Color {
    if depth >= MAX_DEPTH {
        return Vec3::zero();
    }

    let hit = match scene.hit(ray, 0.001, 100000.0) {
        Some(h) => h,
        None => return scene.skybox(ray.dir),
    };

    let mat = hit.material;
    let params = mat.params;
    let base_color = mat.texture.color_at(hit.u, hit.v, hit.face).mul_v(mat.tint);

    if let Some(emissive) = params.emissive {
        let lit = emissive + base_color * 0.25;
        return apply_fog(scene, lit, hit.t);
    }

    let n = if params.has_normal_map {
        let (tu, tv) = face_tangents(hit.face);
        let pert = mat.texture.normal_perturb(hit.u, hit.v);
        (hit.normal + tu * pert.x + tv * pert.y).normalize()
    } else {
        hit.normal
    };

    let light_dir = LIGHT_DIR.normalize();
    let shadow_origin = hit.point + n * 0.001;
    let in_shadow = scene.hit(&Ray::new(shadow_origin, light_dir), 0.001, 100000.0).is_some();

    let ndotl = n.dot(light_dir).max(0.0);
    let sun = if in_shadow { Vec3::zero() } else { base_color * (ndotl * 0.8) };
    let sky_fill_amount = (n.dot(Vec3::new(0.0, 1.0, 0.0)).max(0.0)) * 0.18;
    let sky_fill = base_color.mul_v(Vec3::new(0.55, 0.7, 0.95)) * sky_fill_amount;
    let ambient = base_color * 0.12;

    let view_dir = -ray.dir;
    let half_v = (light_dir + view_dir).normalize();
    let spec_strength = if in_shadow { 0.0 } else { n.dot(half_v).max(0.0).powf(32.0) * params.specular };
    let specular = Vec3::splat(spec_strength);

    let mut color = ambient + sun + sky_fill + specular;

    if params.transparency > 0.0 {
        let front_face = ray.dir.dot(hit.normal) < 0.0;
        let outward_n = if front_face { n } else { n * -1.0 };
        let eta_ratio = if front_face { 1.0 / params.refractive_index } else { params.refractive_index };
        let cos_theta = (-ray.dir.dot(outward_n)).min(1.0);
        let reflect_prob = schlick(cos_theta, params.refractive_index);

        let refracted = ray.dir.refract(outward_n, eta_ratio);
        let refr_color = match refracted {
            Some(refr_dir) => {
                let refr_ray = Ray::new(hit.point - outward_n * 0.001, refr_dir);
                ray_color(scene, &refr_ray, depth + 1)
            }
            None => {
                // reflexion interna total
                let reflect_dir = ray.dir.reflect(outward_n);
                let reflect_ray = Ray::new(hit.point + outward_n * 0.001, reflect_dir);
                ray_color(scene, &reflect_ray, depth + 1)
            }
        };
        let reflect_dir = ray.dir.reflect(outward_n);
        let reflect_ray = Ray::new(hit.point + outward_n * 0.001, reflect_dir);
        let refl_color = ray_color(scene, &reflect_ray, depth + 1);

        let glass_color = Vec3::lerp(refr_color, refl_color, reflect_prob);
        color = Vec3::lerp(color, glass_color, params.transparency);
    } else if params.reflectivity > 0.0 {
        let reflect_dir = ray.dir.reflect(n);
        let reflect_ray = Ray::new(hit.point + n * 0.001, reflect_dir);
        let reflect_color = ray_color(scene, &reflect_ray, depth + 1);
        color = Vec3::lerp(color, reflect_color, params.reflectivity);
    }

    apply_fog(scene, color, hit.t)
}

// Mezcla con el color del cielo segun la distancia, para dar sensacion de profundidad
fn apply_fog(scene: &Scene, color: Color, distance: f32) -> Color {
    const FOG_START: f32 = 14.0;
    const FOG_FULL: f32 = 45.0;
    if distance <= FOG_START {
        return color;
    }
    let t = ((distance - FOG_START) / (FOG_FULL - FOG_START)).clamp(0.0, 0.55);
    let fog_color = scene.skybox(Vec3::new(0.3, 0.15, 0.3));
    Vec3::lerp(color, fog_color, t)
}

// Reparte las filas de la imagen entre varios hilos (uno por nucleo). Devuelve RGBA8.
pub fn render(scene: Arc<Scene>, camera: Camera, width: usize, height: usize) -> Vec<u8> {
    let n_threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(4).max(1);
    let aspect = width as f32 / height as f32;
    let rows_per_thread = (height + n_threads - 1) / n_threads;

    let mut handles = Vec::with_capacity(n_threads);
    for t_idx in 0..n_threads {
        let scene = Arc::clone(&scene);
        let y_start = t_idx * rows_per_thread;
        let y_end = ((t_idx + 1) * rows_per_thread).min(height);
        if y_start >= y_end {
            continue;
        }
        handles.push(thread::spawn(move || {
            let mut rng = Rng::new(0x1234_5678u32.wrapping_add(t_idx as u32 * 7919));
            let mut buf = Vec::with_capacity((y_end - y_start) * width * 4);
            for j in y_start..y_end {
                for i in 0..width {
                    let mut color_acc = Vec3::zero();
                    for _ in 0..SAMPLES_PER_PIXEL {
                        let jx = rng.next_f32() - 0.5;
                        let jy = rng.next_f32() - 0.5;
                        let s = 2.0 * ((i as f32 + 0.5 + jx) / width as f32) - 1.0;
                        let t = 1.0 - 2.0 * ((j as f32 + 0.5 + jy) / height as f32);
                        let ray = camera.get_ray(s, t, aspect);
                        color_acc = color_acc + ray_color(&scene, &ray, 0);
                    }
                    let mut c = (color_acc / SAMPLES_PER_PIXEL as f32).clamp01();
                    c = Vec3::new(c.x.powf(1.0 / 2.2), c.y.powf(1.0 / 2.2), c.z.powf(1.0 / 2.2)); // gamma
                    buf.push((c.x * 255.0) as u8);
                    buf.push((c.y * 255.0) as u8);
                    buf.push((c.z * 255.0) as u8);
                    buf.push(255u8);
                }
            }
            (y_start, buf)
        }));
    }

    let mut pixels = vec![0u8; width * height * 4];
    for h in handles {
        let (y_start, buf) = h.join().expect("un hilo de render fallo");
        let offset = y_start * width * 4;
        pixels[offset..offset + buf.len()].copy_from_slice(&buf);
    }
    pixels
}