use crate::noise::{fbm2, value_noise2};
use crate::vec3::{Color, Vec3};

// Cara de un cubo que fue golpeada por el rayo. Nos sirve para mapear texturas
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Face {
    Top,
    Bottom,
    North,
    South,
    East,
    West,
}

// Devuelve los dos ejes de una cara, en coordenadas del mundo.
pub fn face_tangents(face: Face) -> (Vec3, Vec3) {
    match face {
        Face::Top | Face::Bottom => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
        Face::East | Face::West => (Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 1.0, 0.0)),
        Face::North | Face::South => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
    }
}

// Los distintos "bloques" del diorama, Cada uno tiene su propia textura procedural
#[derive(Clone, Copy)]
pub enum TextureKind {
    Grass,
    Stone,
    Wood,
    Glass,
    Water,
    Lava,
    Leaves,
}

// Parametros fisicos del material
#[derive(Clone, Copy)]
pub struct MaterialParams {
    pub specular: f32,
    pub transparency: f32,
    pub reflectivity: f32,
    pub refractive_index: f32,
    pub emissive: Option<Color>,
    pub has_normal_map: bool,
}

impl TextureKind {
    pub fn params(&self) -> MaterialParams {
        match self {
            TextureKind::Grass => MaterialParams {
                specular: 0.05,
                transparency: 0.0,
                reflectivity: 0.0,
                refractive_index: 1.0,
                emissive: None,
                has_normal_map: false,
            },
            TextureKind::Stone => MaterialParams {
                specular: 0.25,
                transparency: 0.0,
                reflectivity: 0.15,
                refractive_index: 1.0,
                emissive: None,
                has_normal_map: true, // piedra rugosa -> usa mapa de normales
            },
            TextureKind::Wood => MaterialParams {
                specular: 0.08,
                transparency: 0.0,
                reflectivity: 0.0,
                refractive_index: 1.0,
                emissive: None,
                has_normal_map: false,
            },
            TextureKind::Glass => MaterialParams {
                specular: 0.9,
                transparency: 0.85,
                reflectivity: 0.08,
                refractive_index: 1.5, // indice de refraccion del vidrio
                emissive: None,
                has_normal_map: false,
            },
            TextureKind::Water => MaterialParams {
                specular: 0.6,
                transparency: 0.2,
                reflectivity: 0.5, // el agua refleja bastante el cielo
                refractive_index: 1.33,
                emissive: None,
                has_normal_map: false,
            },
            TextureKind::Lava => MaterialParams {
                specular: 0.1,
                transparency: 0.0,
                reflectivity: 0.05,
                refractive_index: 1.0,
                emissive: Some(Vec3::new(1.0, 0.45, 0.05)), // material emisivo
                has_normal_map: false,
            },
            TextureKind::Leaves => MaterialParams {
                specular: 0.05,
                transparency: 0.0,
                reflectivity: 0.0,
                refractive_index: 1.0,
                emissive: None,
                has_normal_map: false,
            },
        }
    }

    // Color base de la textura en el punto
    pub fn color_at(&self, u: f32, v: f32, face: Face) -> Color {
        match self {
            TextureKind::Grass => {
                let dirt = Vec3::new(0.42, 0.27, 0.15);
                let grass_top = Vec3::new(0.30, 0.55, 0.18);
                let n = value_noise2(u * 9.0, v * 9.0, 7) * 0.2;
                match face {
                    Face::Top => Vec3::lerp(grass_top, grass_top * 1.2, n),
                    Face::Bottom => dirt * (0.75 + n * 0.4),
                    _ => {
                        // lados
                        let base = dirt * (0.8 + n * 0.35);
                        if v > 0.85 {
                            Vec3::lerp(base, grass_top, (v - 0.85) / 0.15)
                        } else {
                            base
                        }
                    }
                }
            }
            TextureKind::Stone => {
                let base = Vec3::new(0.5, 0.5, 0.53);
                let n = fbm2(u * 12.0, v * 12.0, 3, 4);
                let grid = block_grid_shade(u, v, 4.0);
                (base * (0.65 + n * 0.55)).mul_v(grid)
            }
            TextureKind::Wood => {
                let base = Vec3::new(0.45, 0.28, 0.14);
                match face {
                    Face::Top | Face::Bottom => {
                        // anillos concentricos vistos desde arriba del tronco
                        let cx = u - 0.5;
                        let cy = v - 0.5;
                        let r = (cx * cx + cy * cy).sqrt();
                        let ring = ((r * 40.0).sin() * 0.5 + 0.5) * 0.25;
                        base * (0.8 + ring)
                    }
                    _ => {
                        let stripes = ((v * 14.0 + (u * 4.0).sin() * 2.0).sin() * 0.5 + 0.5) * 0.22;
                        base * (0.82 + stripes)
                    }
                }
            }
            TextureKind::Glass => {
                let tint = Vec3::new(0.75, 0.9, 0.95);
                let grid = block_grid_shade(u, v, 2.0);
                Vec3::lerp(tint, Vec3::splat(1.0), 0.0) * (0.85 + 0.15 * grid.x)
            }
            TextureKind::Water => {
                let base = Vec3::new(0.15, 0.35, 0.55);
                let n = value_noise2(u * 7.0 + 3.0, v * 7.0 + 3.0, 21);
                base * (0.85 + n * 0.35)
            }
            TextureKind::Lava => {
                let n = fbm2(u * 6.0, v * 6.0, 55, 3);
                Vec3::lerp(Vec3::new(0.75, 0.12, 0.0), Vec3::new(1.0, 0.8, 0.15), n)
            }
            TextureKind::Leaves => {
                // Follaje moteado en todas las caras
               
                let base = Vec3::new(0.28, 0.5, 0.16);
                let n = fbm2(u * 14.0 + 5.0, v * 14.0 + 5.0, 88, 3);
                let clump = value_noise2(u * 5.0, v * 5.0, 12);
                let shaded = Vec3::lerp(base * 0.75, base * 1.25, n);
                Vec3::lerp(shaded, shaded * 0.7, (clump < 0.25) as i32 as f32)
            }
        }
    }

    // Perturbacion del mapa de normales 
    pub fn normal_perturb(&self, u: f32, v: f32) -> Vec3 {
        match self {
            TextureKind::Stone => {
                let e = 0.04;
                let h = |uu: f32, vv: f32| fbm2(uu * 12.0, vv * 12.0, 3, 4);
                let dh_du = (h(u + e, v) - h(u - e, v)) / (2.0 * e);
                let dh_dv = (h(u, v + e) - h(u, v - e)) / (2.0 * e);
                Vec3::new(-dh_du, -dh_dv, 0.0) * 0.7
            }
            _ => Vec3::zero(),
        }
    }
}

// Dibuja lineas finas de mortero para que se note la division entre bloques
fn block_grid_shade(u: f32, v: f32, subdivisions: f32) -> Color {
    let fu = (u * subdivisions).fract();
    let fv = (v * subdivisions).fract();
    let edge = 0.05;
    if fu < edge || fu > 1.0 - edge || fv < edge || fv > 1.0 - edge {
        Vec3::splat(0.82)
    } else {
        Vec3::splat(1.0)
    }
}