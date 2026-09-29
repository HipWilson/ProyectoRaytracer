// Vector de 3 componentes, usado tanto para posiciones/direcciones como para colores RGB (0.0..1.0).
use std::ops::{Add, Sub, Mul, Div, Neg};

#[derive(Clone, Copy, Debug)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

// Reutilizamos Vec3 como color (r,g,b) para no duplicar código.
pub type Color = Vec3;

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }

    pub fn zero() -> Self {
        Vec3::new(0.0, 0.0, 0.0)
    }

    pub fn splat(v: f32) -> Self {
        Vec3::new(v, v, v)
    }

    pub fn dot(&self, other: Vec3) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(&self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length_squared(&self) -> f32 {
        self.dot(*self)
    }

    pub fn length(&self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn normalize(&self) -> Vec3 {
        let len = self.length();
        if len > 1e-6 {
            *self * (1.0 / len)
        } else {
            *self
        }
    }

    // Multiplicación componente a componente (para combinar colores).
    pub fn mul_v(&self, other: Vec3) -> Vec3 {
        Vec3::new(self.x * other.x, self.y * other.y, self.z * other.z)
    }

    pub fn lerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
        a * (1.0 - t) + b * t
    }

    pub fn clamp01(&self) -> Vec3 {
        Vec3::new(self.x.clamp(0.0, 1.0), self.y.clamp(0.0, 1.0), self.z.clamp(0.0, 1.0))
    }

    // Refleja un vector (self) respecto a una normal.
    pub fn reflect(&self, normal: Vec3) -> Vec3 {
        *self - normal * (2.0 * self.dot(normal))
    }

    // Refracta un vector (self, se asume ya normalizado) usando la ley de Snell.
    // `normal` debe apuntar "contra" el rayo entrante (hacia afuera del material que atraviesa).
    // `eta_ratio` = indice_de_refraccion_origen / indice_de_refraccion_destino
    pub fn refract(&self, normal: Vec3, eta_ratio: f32) -> Option<Vec3> {
        let uv = self.normalize();
        let cos_theta = (-uv.dot(normal)).min(1.0);
        let r_out_perp = (uv + normal * cos_theta) * eta_ratio;
        let k = 1.0 - r_out_perp.length_squared();
        if k < 0.0 {
            None // reflexion interna total, no hay rayo refractado posible
        } else {
            let r_out_parallel = normal * -(k.sqrt());
            Some(r_out_perp + r_out_parallel)
        }
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Div<f32> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f32) -> Vec3 {
        Vec3::new(self.x / s, self.y / s, self.z / s)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}
