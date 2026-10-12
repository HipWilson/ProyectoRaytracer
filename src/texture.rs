use crate::noise::{fbm2, hash2, value_noise2};
use crate::vec3::{Color, Vec3};

// Cara de un cubo que fue golpeada por el rayo. Nos sirve para mapear texturas.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Face {
    Top,
    Bottom,
    North,
    South,
    East,
    West,
}

// Devuelve los dos ejes (u, v) de una cara, en coordenadas del mundo.
// Coinciden con como cube.rs calcula (u, v), asi que sirven para el mapa de normales.
pub fn face_tangents(face: Face) -> (Vec3, Vec3) {
    match face {
        Face::Top | Face::Bottom => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
        Face::East | Face::West => (Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 1.0, 0.0)),
        Face::North | Face::South => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
    }
}

// Los distintos "bloques" del diorama. Cada uno tiene su propia textura procedural.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TextureKind {
    Grass,
    Dirt,
    Sand,
    Stone,
    Wood, // tablones de madera
    Log,  // tronco
    Glass,
    Water,
    Lava,
    Leaves,
    Gold,
    Glowstone,
}

// Parametros fisicos del material.
#[derive(Clone, Copy)]
pub struct MaterialParams {
    pub albedo: f32,        // cuanta luz difusa refleja (0..1)
    pub specular: f32,      // intensidad del brillo especular
    pub shininess: f32,     // que tan "filoso" es el brillo
    pub transparency: f32,  // 0 = opaco, 1 = totalmente transparente
    pub reflectivity: f32,  // reflexion tipo espejo (materiales opacos, ej. oro)
    pub refractive_index: f32,
    pub absorption: f32,    // cuanto se oscurece/tine la luz al viajar por dentro (agua profunda)
    pub emissive: Option<Color>,
    pub has_normal_map: bool,
    pub metallic: bool,     // el reflejo se tine con el color del material
}

fn base_params() -> MaterialParams {
    MaterialParams {
        albedo: 0.9,
        specular: 0.05,
        shininess: 24.0,
        transparency: 0.0,
        reflectivity: 0.0,
        refractive_index: 1.0,
        absorption: 0.0,
        emissive: None,
        has_normal_map: false,
        metallic: false,
    }
}

// ---- utilidades para texturas estilo "pixel art" de Minecraft ----
#[inline]
fn frac(x: f32) -> f32 {
    x - x.floor()
}
// Ruido constante dentro de cada "texel" (pixel grande) de la textura. `res` = texeles por bloque.
// `d` es el nivel de detalle (LOD): 1 = textura completa (cerca), 0 = promedio liso (muy lejos).
// Cuando un texel es mas chico que un pixel de la pantalla, el ruido solo produce parpadeo/pixeleado;
// por eso se mezcla hacia el valor medio segun la distancia (como el mipmapping de una GPU, pero en CPU).
#[inline]
fn texel(d: f32, u: f32, v: f32, res: f32, seed: u32) -> f32 {
    let n = hash2((u * res).floor() as i32, (v * res).floor() as i32, seed);
    0.5 + (n - 0.5) * d
}
#[inline]
fn smooth01(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
// Oscurece levemente los bordes de cada bloque para que se note que son cubos individuales.
#[inline]
fn block_edge(u: f32, v: f32) -> f32 {
    let e = frac(u).min(1.0 - frac(u)).min(frac(v)).min(1.0 - frac(v));
    if e < 0.035 { 0.86 } else { 1.0 }
}

fn dirt_color(d: f32, u: f32, v: f32) -> Color {
    let n = texel(d, u, v, 8.0, 17);
    let mut c = Vec3::new(0.44, 0.30, 0.18) * (0.72 + 0.45 * n);
    if texel(d, u, v, 8.0, 18) > 0.92 {
        c = Vec3::new(0.40, 0.38, 0.36); // piedritas
    }
    c
}

// Ladrillos de piedra: devuelve (es_mortero, tono_del_ladrillo, altura_para_normal_map).
// Ladrillos de 1 bloque de ancho x 0.5 de alto, con las filas desfasadas.
fn brick_pattern(u: f32, v: f32) -> (bool, f32, f32) {
    let row = (v * 2.0).floor();
    let off = if (row as i32) % 2 == 0 { 0.0 } else { 0.5 };
    let bu = frac(u + off);
    let bv = frac(v * 2.0);
    // distancia al borde del ladrillo (en bloques); el mortero mide ~0.05
    let d = bu.min(1.0 - bu).min(bv.min(1.0 - bv) * 0.5);
    let mortar = d < 0.05;
    let id = hash2((u + off).floor() as i32, row as i32, 3);
    let height = smooth01(0.02, 0.10, d);
    (mortar, id, height)
}

// Tablones de madera horizontales: (es_junta, tono, altura).
fn plank_pattern(u: f32, v: f32) -> (bool, f32, f32) {
    let board = (v * 4.0).floor();
    let bv = frac(v * 4.0);
    let seam_pos = frac(u * 0.5 + board * 0.37);
    let d_edge = bv.min(1.0 - bv) * 0.25; // en bloques
    let d_seam = seam_pos.min(1.0 - seam_pos) * 2.0;
    let d = d_edge.min(d_seam);
    let joint = d < 0.012;
    let tone = hash2(board as i32, 5, 9);
    (joint, tone, smooth01(0.0, 0.04, d))
}

impl TextureKind {
    pub fn params(&self) -> MaterialParams {
        let b = base_params();
        match self {
            TextureKind::Grass => MaterialParams { albedo: 0.9, specular: 0.04, ..b },
            TextureKind::Dirt => MaterialParams { albedo: 0.85, specular: 0.02, ..b },
            TextureKind::Sand => MaterialParams { albedo: 0.95, specular: 0.06, has_normal_map: true, ..b },
            TextureKind::Stone => MaterialParams {
                albedo: 0.85,
                specular: 0.22,
                shininess: 20.0,
                has_normal_map: true, // ladrillos con relieve (mapa de normales)
                ..b
            },
            TextureKind::Wood => MaterialParams { albedo: 0.9, specular: 0.08, has_normal_map: true, ..b },
            TextureKind::Log => MaterialParams { albedo: 0.85, specular: 0.04, ..b },
            TextureKind::Glass => MaterialParams {
                albedo: 0.15,
                specular: 0.95,
                shininess: 140.0,
                transparency: 0.88,
                reflectivity: 0.0, // el reflejo del vidrio sale de Fresnel (ver render.rs)
                refractive_index: 1.5,
                absorption: 0.0,
                ..b
            },
            TextureKind::Water => MaterialParams {
                albedo: 0.35,
                specular: 0.9,
                shininess: 160.0,
                transparency: 0.82,
                refractive_index: 1.33,
                absorption: 0.9,
                has_normal_map: true, // ondas animadas
                ..b
            },
            TextureKind::Lava => MaterialParams {
                albedo: 0.0,
                specular: 0.1,
                emissive: Some(Vec3::new(1.0, 0.42, 0.05)),
                ..b
            },
            TextureKind::Leaves => MaterialParams { albedo: 0.9, specular: 0.03, ..b },
            TextureKind::Gold => MaterialParams {
                albedo: 0.55,
                specular: 0.9,
                shininess: 70.0,
                reflectivity: 0.55,
                metallic: true,
                ..b
            },
            TextureKind::Glowstone => MaterialParams {
                albedo: 0.0,
                specular: 0.0,
                emissive: Some(Vec3::new(1.0, 0.75, 0.35)),
                ..b
            },
        }
    }

    // Color base de la textura en el punto. (u, v) estan en unidades de bloque del mundo.
    // `time` solo lo usan los materiales animados (lava). `detail` (0..1) controla cuanto ruido fino se muestra (LOD).
    pub fn color_at(&self, u: f32, v: f32, face: Face, time: f32, detail: f32) -> Color {
        let d = detail.clamp(0.0, 1.0);
        match self {
            TextureKind::Grass => match face {
                Face::Top => {
                    let n = texel(d, u, v, 8.0, 7);
                    let m = texel(d, u, v, 2.0, 9) * 0.18;
                    Vec3::new(0.30, 0.56, 0.17) * (0.70 + 0.50 * n + m) * block_edge(u, v)
                }
                Face::Bottom => dirt_color(d, u, v),
                _ => {
                    // Lado del bloque de pasto: franja verde irregular arriba, tierra abajo.
                    let lv = frac(v);
                    let depth = 0.74 + 0.12 * texel(d, u, 1.0, 8.0, 13);
                    let c = if lv > depth {
                        Vec3::new(0.27, 0.50, 0.15) * (0.75 + 0.4 * texel(d, u, v, 8.0, 14))
                    } else {
                        dirt_color(d, u, v)
                    };
                    c * block_edge(u, v)
                }
            },
            TextureKind::Dirt => dirt_color(d, u, v) * block_edge(u, v),
            TextureKind::Sand => {
                let n = texel(d, u, v, 8.0, 31);
                Vec3::new(0.86, 0.78, 0.52) * (0.88 + 0.22 * n) * block_edge(u, v)
            }
            TextureKind::Stone => {
                let (mortar, id, _) = brick_pattern(u, v);
                let mortar_c = Vec3::new(0.30, 0.30, 0.33);
                let n = texel(d, u, v, 8.0, 5);
                let brick_c = Vec3::new(0.52, 0.52, 0.55) * (0.78 + 0.28 * id) * (0.9 + 0.2 * n);
                let sharp = if mortar { mortar_c } else { brick_c };
                // LOD: de lejos las juntas finitas se promedian con el ladrillo (evita puntos/moire)
                let avg = Vec3::lerp(brick_c, mortar_c, 0.15);
                Vec3::lerp(avg, sharp, d)
            }
            TextureKind::Wood => {
                let (joint, tone, _) = plank_pattern(u, v);
                let joint_c = Vec3::new(0.25, 0.15, 0.07);
                let grain = texel(d, u * 0.25, v, 16.0, 21);
                let plank_c = Vec3::new(0.64, 0.46, 0.26) * (0.80 + 0.28 * tone) * (0.92 + 0.14 * grain);
                let sharp = if joint { joint_c } else { plank_c };
                let avg = Vec3::lerp(plank_c, joint_c, 0.10);
                Vec3::lerp(avg, sharp, d)
            }
            TextureKind::Log => match face {
                Face::Top | Face::Bottom => {
                    let r = ((frac(u) - 0.5).powi(2) + (frac(v) - 0.5).powi(2)).sqrt();
                    let ring = ((r * 34.0).sin() * 0.5 + 0.5) * 0.28;
                    if r > 0.42 {
                        Vec3::new(0.30, 0.20, 0.10)
                    } else {
                        Vec3::new(0.70, 0.52, 0.30) * (0.82 + ring)
                    }
                }
                _ => {
                    let stripe = texel(d, u, 0.5, 6.0, 41);
                    let fleck = texel(d, u, v, 8.0, 42);
                    Vec3::new(0.32, 0.21, 0.11) * (0.65 + 0.55 * stripe) * (0.9 + 0.2 * fleck)
                }
            },
            TextureKind::Glass => {
                let e = frac(u).min(1.0 - frac(u)).min(frac(v)).min(1.0 - frac(v));
                if e < 0.07 {
                    Vec3::new(0.92, 0.98, 1.0) // marco
                } else {
                    Vec3::new(0.72, 0.90, 0.96) * (0.96 + 0.06 * texel(d, u, v, 4.0, 51))
                }
            }
            TextureKind::Water => {
                let n = value_noise2(u * 2.5 + 3.0, v * 2.5 + 3.0, 21);
                Vec3::new(0.08, 0.36, 0.58) * (0.85 + n * 0.30)
            }
            TextureKind::Lava => {
                // Magma que "fluye" lentamente con el tiempo.
                let n = fbm2(u * 1.6 + time * 0.10, v * 1.6 - time * 0.07, 55, 4);
                let crack = smooth01(0.38, 0.70, n);
                let dark = Vec3::new(0.50, 0.05, 0.0);
                let mid = Vec3::new(1.0, 0.38, 0.02);
                let hot = Vec3::new(1.0, 0.88, 0.25);
                if crack < 0.5 {
                    Vec3::lerp(dark, mid, crack * 2.0)
                } else {
                    Vec3::lerp(mid, hot, (crack - 0.5) * 2.0)
                }
            }
            TextureKind::Leaves => {
                let n = texel(d, u, v, 8.0, 61);
                let hole = texel(d, u, v, 5.0, 62);
                let base = Vec3::new(0.28, 0.52, 0.16);
                if hole < 0.16 {
                    Vec3::new(0.10, 0.22, 0.07)
                } else {
                    base * (0.70 + 0.55 * n)
                }
            }
            TextureKind::Gold => {
                let e = frac(u).min(1.0 - frac(u)).min(frac(v)).min(1.0 - frac(v));
                let n = texel(d, u, v, 8.0, 71);
                let c = Vec3::new(1.0, 0.77, 0.26) * (0.82 + 0.28 * n);
                if e < 0.07 { c * 0.78 } else if e < 0.14 { c * 1.12 } else { c }
            }
            TextureKind::Glowstone => {
                let n = texel(d, u, v, 4.0, 81);
                let cell = texel(d, u, v, 2.0, 82);
                Vec3::new(1.0, 0.82, 0.45) * (0.65 + 0.45 * n) * (0.85 + 0.2 * cell)
            }
        }
    }

    // "Altura" de la superficie, de la que sale el mapa de normales (por diferencias finitas).
    fn height(&self, u: f32, v: f32, time: f32) -> f32 {
        match self {
            TextureKind::Stone => {
                let (_, _, h) = brick_pattern(u, v);
                h * 0.9 + 0.15 * value_noise2(u * 14.0, v * 14.0, 2)
            }
            TextureKind::Wood => {
                let (_, _, h) = plank_pattern(u, v);
                h * 0.7 + 0.12 * value_noise2(u * 3.0, v * 28.0, 4)
            }
            TextureKind::Sand => {
                // dunitas: ondulaciones suaves
                (u * 7.0 + (v * 2.6).sin() * 2.0).sin() * 0.5 + 0.5 + 0.2 * value_noise2(u * 12.0, v * 12.0, 8)
            }
            TextureKind::Water => {
                // ondas animadas que se desplazan con el tiempo
                ((u * 2.7 + time * 1.3).sin() + (v * 3.3 - time * 1.1 + u * 0.7).sin()) * 0.5
                    + 0.35 * (u * 6.1 - v * 5.3 + time * 1.9).sin()
            }
            _ => 0.0,
        }
    }

    fn bump_strength(&self) -> f32 {
        match self {
            TextureKind::Stone => 0.55,
            TextureKind::Wood => 0.35,
            TextureKind::Sand => 0.06,
            TextureKind::Water => 0.045,
            _ => 0.0,
        }
    }

    // Perturbacion de la normal (en el espacio tangente de la cara) -> MAPA DE NORMALES.
    // x = inclinacion a lo largo de u, y = inclinacion a lo largo de v.
    pub fn normal_perturb(&self, u: f32, v: f32, time: f32, detail: f32) -> Vec3 {
        // el relieve fino tambien se suaviza con la distancia (menos "moire" y parpadeo)
        let s = self.bump_strength() * detail.clamp(0.0, 1.0).max(if matches!(self, TextureKind::Water) { 1.0 } else { 0.0 });
        if s == 0.0 {
            return Vec3::zero();
        }
        let e = 0.012;
        let dh_du = (self.height(u + e, v, time) - self.height(u - e, v, time)) / (2.0 * e);
        let dh_dv = (self.height(u, v + e, time) - self.height(u, v - e, time)) / (2.0 * e);
        Vec3::new(-dh_du * s, -dh_dv * s, 0.0)
    }
}