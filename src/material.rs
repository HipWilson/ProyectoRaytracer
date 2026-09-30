use crate::texture::{MaterialParams, TextureKind};
use crate::vec3::Color;
use crate::vec3::Vec3;

#[derive(Clone, Copy)]
pub struct Material {
    pub texture: TextureKind,
    pub params: MaterialParams,
    // Multiplicador de color (1,1,1 = sin cambio). Permite reusar la misma textura
    // procedural con variaciones de color distintas (por ejemplo, hojas mas amarillas
    // o mas anaranjadas en distintos arboles) sin tener que escribir una textura nueva.
    pub tint: Color,
}

impl Material {
    pub fn new(texture: TextureKind) -> Self {
        let params = texture.params();
        Material { texture, params, tint: Vec3::splat(1.0) }
    }

    pub fn new_tinted(texture: TextureKind, tint: Color) -> Self {
        let params = texture.params();
        Material { texture, params, tint }
    }
}