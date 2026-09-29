use std::sync::Arc;
use std::thread;

use crate::camera::Camera;
use crate::ray::Ray;
use crate::scene::Scene;
use crate::texture::face_tangents;
use crate::vec3::{Color, Vec3};

const MAX_DEPTH: u32 = 4;
// Direccion HACIA la luz (el sol). No es la direccion en la que viaja la luz, sino de donde viene.
const LIGHT_DIR: Vec3 = Vec3 { x: 0.45, y: 0.85, z: 0.35 };

// Aproximacion de Schlick para mezclar reflexion/refraccion segun el angulo (efecto Fresnel):
// mirando un vidrio de frente se ve mas transparente, mirandolo "de canto" se ve mas como espejo.
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
    let base_color = mat.texture.color_at(hit.u, hit.v, hit.face);

    // Material emisivo (la lava): brilla con su propio color, no depende de la luz externa.
    if let Some(emissive) = params.emissive {
        return emissive + base_color * 0.25;
    }

    // Mapa de normales: perturbamos la normal geometrica usando la textura de relieve.
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

    let ambient = base_color * 0.2;
    let ndotl = n.dot(light_dir).max(0.0);
    let diffuse = if in_shadow { Vec3::zero() } else { base_color * (ndotl * 0.8) };

    let view_dir = -ray.dir;
    let half_v = (light_dir + view_dir).normalize();
    let spec_strength = if in_shadow { 0.0 } else { n.dot(half_v).max(0.0).powf(32.0) * params.specular };
    let specular = Vec3::splat(spec_strength);

    let mut color = ambient + diffuse + specular;

    if params.transparency > 0.0 {
        // Vidrio / agua: mezclamos un rayo refractado y uno reflejado segun Fresnel.
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
            // reflexion interna total: no hay refraccion posible, se comporta como espejo
            None => {
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

    color
}

// Renderiza la escena completa y devuelve un buffer de pixeles RGBA8 (4 bytes por pixel),
// que es el formato que usaremos para subirlo directamente a una textura de raylib.
//
// Optimizacion de paralelismo: dividimos la imagen en franjas horizontales y cada hilo del
// sistema operativo (uno por nucleo disponible) renderiza su franja de forma independiente;
// no comparten memoria mutable entre si, asi que no hace falta ningun candado (mutex).
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
            let mut buf = Vec::with_capacity((y_end - y_start) * width * 4);
            for j in y_start..y_end {
                for i in 0..width {
                    let s = 2.0 * ((i as f32 + 0.5) / width as f32) - 1.0;
                    let t = 1.0 - 2.0 * ((j as f32 + 0.5) / height as f32);
                    let ray = camera.get_ray(s, t, aspect);
                    let c = ray_color(&scene, &ray, 0).clamp01();
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
