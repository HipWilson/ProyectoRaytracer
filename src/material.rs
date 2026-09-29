use crate::texture::{MaterialParams, TextureKind};

#[derive(Clone, Copy)]
pub struct Material {
    pub texture: TextureKind,
    pub params: MaterialParams,
}

impl Material {
    pub fn new(texture: TextureKind) -> Self {
        let params = texture.params();
        Material { texture, params }
    }
}
