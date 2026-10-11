use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crate::camera::Camera;
use crate::ray::Ray;
use crate::scene::Scene;
use crate::sky::Environment;
use crate::texture::face_tangents;
use crate::vec3::{Color, Vec3};

const MAX_DEPTH: u32 = 5;

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

// Devuelve (color, distancia recorrida hasta el primer golpe). La distancia sirve para la
// absorcion del agua (mientras mas profundo, mas azul oscuro).
fn trace(scene: &Scene, env: &Environment, ray: &Ray, depth: u32) -> (Color, f32) {
    if depth >= MAX_DEPTH {
        return (scene.sky_color(ray.dir, env), f32::INFINITY);
    }

    let hit = match scene.hit(ray, 0.001, 100000.0) {
        Some(h) => h,
        None => return (scene.sky_color(ray.dir, env), f32::INFINITY),
    };

    let mat = hit.material;
    let params = mat.params;
    let base = mat.texture.color_at(hit.u, hit.v, hit.face, env.anim_time).mul_v(mat.tint);

    // Material EMISIVO (lava, glowstone): brilla con luz propia, no depende de la iluminacion.
    if let Some(emissive) = params.emissive {
        let lit = base * 1.15 + emissive * 0.30;
        return (apply_fog(env, lit, hit.t), hit.t);
    }

    // MAPA DE NORMALES: inclinamos la normal segun la textura
    let n = if params.has_normal_map {
        let (tu, tv) = face_tangents(hit.face);
        let pert = mat.texture.normal_perturb(hit.u, hit.v, env.anim_time);
        (hit.normal + tu * pert.x + tv * pert.y).normalize()
    } else {
        hit.normal
    };

    let view_dir = -ray.dir;
    let shadow_origin = hit.point + hit.normal * 0.002;

    // ---- Luz principal (sol de dia / luna de noche) ----
    let l = env.light_dir;
    let ndotl = n.dot(l).max(0.0);
    let mut diffuse = Vec3::zero();
    let mut specular = Vec3::zero();
    if ndotl > 0.0 && env.light_color.length_squared() > 1e-5 {
        if !scene.occluded(&Ray::new(shadow_origin, l), 100000.0) {
            diffuse = diffuse + base.mul_v(env.light_color) * ndotl;
            let h = (l + view_dir).normalize();
            let s = n.dot(h).max(0.0).powf(params.shininess) * params.specular;
            specular = specular + env.light_color * s;
        }
    }

    // ---- Luces puntuales (antorchas, lamparas, lava) con sombras ----
    for light in &scene.lights {
        let to_l = light.pos - hit.point;
        let d2 = to_l.length_squared();
        if d2 > light.radius * light.radius {
            continue;
        }
        let dist = d2.sqrt();
        let ld = to_l / dist;
        let nl = n.dot(ld).max(0.0);
        if nl <= 0.0 {
            continue;
        }
        let atten = (1.0 - dist / light.radius).powi(2);
        if scene.occluded(&Ray::new(shadow_origin, ld), dist - 0.05) {
            continue;
        }
        diffuse = diffuse + base.mul_v(light.color) * (nl * atten * 0.9);
        let h = (ld + view_dir).normalize();
        specular = specular + light.color * (n.dot(h).max(0.0).powf(params.shininess) * params.specular * atten);
    }

    // ---- Luz ambiental del cielo (hemisferica: mas luz hacia arriba) ----
    let hemi = 0.55 + 0.45 * n.y;
    let ambient = base.mul_v(env.ambient) * hemi;

    let mut color = (diffuse + ambient) * params.albedo / 0.9 + specular;

    if params.transparency > 0.0 {
        // ---- Material transparente (vidrio, agua): refraccion + reflexion mezcladas con Fresnel ----
        let front_face = ray.dir.dot(hit.normal) < 0.0;
        let outward_n = if front_face { n } else { n * -1.0 };
        let eta_ratio = if front_face { 1.0 / params.refractive_index } else { params.refractive_index };
        let cos_theta = (-ray.dir.dot(outward_n)).clamp(0.0, 1.0);
        let reflect_prob = schlick(cos_theta, params.refractive_index);

        // Refraccion (ley de Snell). Si hay reflexion interna total, todo se refleja.
        let mut transmitted = Vec3::zero();
        let mut tir = false;
        if reflect_prob < 0.995 {
            match ray.dir.refract(outward_n, eta_ratio) {
                Some(refr_dir) => {
                    let refr_ray = Ray::new(hit.point - outward_n * 0.002, refr_dir);
                    let (c, d) = trace(scene, env, &refr_ray, depth + 1);
                    transmitted = c;
                    // Absorcion en el agua: lo que esta mas profundo se ve mas oscuro y azul.
                    if params.absorption > 0.0 && front_face {
                        let dd = if d.is_finite() { d } else { 30.0 };
                        let w = (-params.absorption * dd).exp();
                        let deep = base * (0.10 + 0.35 * env.day_amt);
                        transmitted = Vec3::lerp(deep, c, w);
                    }
                }
                None => tir = true,
            }
        } else {
            tir = true;
        }

        let reflect_dir = ray.dir.reflect(outward_n);
        let refl_ray = Ray::new(hit.point + outward_n * 0.002, reflect_dir);
        let (reflected, _) = if tir || reflect_prob > 0.015 {
            trace(scene, env, &refl_ray, depth + 1)
        } else {
            (Vec3::zero(), 0.0)
        };

        let mix = if tir { 1.0 } else { reflect_prob };
        let through = Vec3::lerp(transmitted, reflected, mix);
        // el color propio del material (tinte) solo afecta lo que lo atraviesa
        let tinted = if tir { through } else { through.mul_v(Vec3::lerp(Vec3::splat(1.0), base, 0.35)) };
        let body = (diffuse + ambient) * params.albedo / 0.9;
        color = Vec3::lerp(body, tinted, params.transparency) + specular;
    } else if params.reflectivity > 0.0 {
        // ---- Reflexion tipo espejo (oro) ----
        let reflect_dir = ray.dir.reflect(n);
        let refl_ray = Ray::new(hit.point + hit.normal * 0.002, reflect_dir);
        let (mut rc, _) = trace(scene, env, &refl_ray, depth + 1);
        if params.metallic {
            rc = rc.mul_v(Vec3::lerp(Vec3::splat(1.0), base, 0.8));
        }
        color = Vec3::lerp(color, rc, params.reflectivity);
    }

    (apply_fog(env, color, hit.t), hit.t)
}

// Mezcla con el color del horizonte segun la distancia, para dar sensacion de profundidad
fn apply_fog(env: &Environment, color: Color, distance: f32) -> Color {
    const FOG_START: f32 = 30.0;
    const FOG_FULL: f32 = 90.0;
    if distance <= FOG_START {
        return color;
    }
    let t = ((distance - FOG_START) / (FOG_FULL - FOG_START)).clamp(0.0, 0.5);
    Vec3::lerp(color, env.fog_color, t)
}

#[inline]
fn to_u8(c: f32) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

// Reparte las filas de la imagen entre varios hilos (uno por nucleo) con una cola de trabajo
// compartida (contador atomico): cada hilo toma la siguiente fila libre apenas termina la suya.
// Asi ningun hilo queda ocioso aunque unas filas (con vidrio/agua/reflejos) cuesten mas que otras.
// `samples` = rayos por pixel (anti-aliasing con muestreo estratificado). Devuelve RGBA8.
pub fn render(scene: &Scene, camera: &Camera, env: &Environment, width: usize, height: usize, samples: u32) -> Vec<u8> {
    let n_threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(4).max(1);
    let aspect = width as f32 / height as f32;
    let samples = samples.max(1);
    let grid = (samples as f32).sqrt().ceil() as u32; // rejilla de estratos grid x grid
    let next_row = AtomicUsize::new(0);

    let mut pixels = vec![0u8; width * height * 4];

    let results: Vec<Vec<(usize, Vec<u8>)>> = thread::scope(|s| {
        let handles: Vec<_> = (0..n_threads)
            .map(|_| {
                s.spawn(|| {
                    let mut rows: Vec<(usize, Vec<u8>)> = Vec::new();
                    loop {
                        let j = next_row.fetch_add(1, Ordering::Relaxed);
                        if j >= height {
                            break;
                        }
                        // semilla por fila -> el resultado es identico en cada render (sin parpadeo)
                        let mut rng = Rng::new(0x1234_5678u32 ^ (j as u32).wrapping_mul(2654435761));
                        let mut buf = Vec::with_capacity(width * 4);
                        for i in 0..width {
                            let mut acc = Vec3::zero();
                            for k in 0..samples {
                                let (jx, jy) = if samples == 1 {
                                    (0.0, 0.0)
                                } else {
                                    let cx = (k % grid) as f32;
                                    let cy = (k / grid) as f32;
                                    (
                                        (cx + rng.next_f32()) / grid as f32 - 0.5,
                                        (cy + rng.next_f32()) / grid as f32 - 0.5,
                                    )
                                };
                                let sx = 2.0 * ((i as f32 + 0.5 + jx) / width as f32) - 1.0;
                                let sy = 1.0 - 2.0 * ((j as f32 + 0.5 + jy) / height as f32);
                                let ray = camera.get_ray(sx, sy, aspect);
                                acc = acc + trace(scene, env, &ray, 0).0;
                            }
                            let c = (acc / samples as f32).clamp01();
                            let g = 1.0 / 2.2; // gamma
                            buf.push(to_u8(c.x.powf(g)));
                            buf.push(to_u8(c.y.powf(g)));
                            buf.push(to_u8(c.z.powf(g)));
                            buf.push(255u8);
                        }
                        rows.push((j, buf));
                    }
                    rows
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("un hilo de render fallo")).collect()
    });

    for rows in results {
        for (j, buf) in rows {
            let off = j * width * 4;
            pixels[off..off + buf.len()].copy_from_slice(&buf);
        }
    }
    pixels
}

// Agranda una imagen RGBA8 por vecino mas cercano (se usa para mostrar el preview de baja
// resolucion en la textura de tamano completo mientras la camara se mueve).
pub fn upscale_nearest(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 4];
    for y in 0..dh {
        let sy = (y * sh / dh).min(sh - 1);
        for x in 0..dw {
            let sx = (x * sw / dw).min(sw - 1);
            let si = (sy * sw + sx) * 4;
            let di = (y * dw + x) * 4;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    out
}

// Agranda una imagen RGBA8 con interpolacion BILINEAL (suaviza en vez de mostrar pixeles cuadrados).
// Se hace en la CPU, asi la imagen final ya sale al tamano de la ventana y raylib solo la dibuja 1:1.
pub fn upscale_bilinear(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 4];
    // Precalculamos, para cada columna destino, los dos pixeles origen y el peso (se reutiliza en todas las filas)
    let mut xs: Vec<(usize, usize, u32)> = Vec::with_capacity(dw);
    for x in 0..dw {
        let fx = ((x as f32 + 0.5) * sw as f32 / dw as f32 - 0.5).clamp(0.0, (sw - 1) as f32);
        let x0 = fx.floor() as usize;
        let x1 = (x0 + 1).min(sw - 1);
        xs.push((x0, x1, ((fx - x0 as f32) * 256.0) as u32));
    }
    for y in 0..dh {
        let fy = ((y as f32 + 0.5) * sh as f32 / dh as f32 - 0.5).clamp(0.0, (sh - 1) as f32);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(sh - 1);
        let wy = ((fy - y0 as f32) * 256.0) as u32;
        for (x, &(x0, x1, wx)) in xs.iter().enumerate() {
            let i00 = (y0 * sw + x0) * 4;
            let i10 = (y0 * sw + x1) * 4;
            let i01 = (y1 * sw + x0) * 4;
            let i11 = (y1 * sw + x1) * 4;
            let o = (y * dw + x) * 4;
            for c in 0..3 {
                let top = src[i00 + c] as u32 * (256 - wx) + src[i10 + c] as u32 * wx;
                let bot = src[i01 + c] as u32 * (256 - wx) + src[i11 + c] as u32 * wx;
                out[o + c] = ((top * (256 - wy) + bot * wy) >> 16) as u8;
            }
            out[o + 3] = 255;
        }
    }
    out
}