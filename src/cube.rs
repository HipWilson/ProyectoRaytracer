use crate::material::Material;
use crate::ray::Ray;
use crate::texture::Face;
use crate::vec3::Vec3;

pub struct Cube {
    pub min: Vec3,
    pub max: Vec3,
    pub material: Material,
}

pub struct Hit {
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub face: Face,
    pub u: f32,
    pub v: f32,
    pub material: Material,
}

impl Cube {
    pub fn new(min: Vec3, max: Vec3, material: Material) -> Self {
        Cube { min, max, material }
    }

    // Metodo de "slabs": la tecnica clasica para intersectar un rayo con una caja alineada a los ejes.
    // Ademas de t, calculamos cual de las 3 dimensiones (x/y/z) fue la que determino la entrada,
    // para saber en que cara del cubo golpeamos y poder mapear la textura ahi.
    pub fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        let orig = [ray.origin.x, ray.origin.y, ray.origin.z];
        let dir = [ray.dir.x, ray.dir.y, ray.dir.z];
        let bmin = [self.min.x, self.min.y, self.min.z];
        let bmax = [self.max.x, self.max.y, self.max.z];

        let mut tmin = t_min;
        let mut tmax = t_max;
        let mut hit_axis: usize = 0;
        let mut hit_sign: f32 = 1.0;

        for axis in 0..3 {
            if dir[axis].abs() < 1e-8 {
                // Rayo paralelo a este eje: si el origen esta fuera del rango de la caja, no hay interseccion.
                if orig[axis] < bmin[axis] || orig[axis] > bmax[axis] {
                    return None;
                }
                continue;
            }
            let inv_d = 1.0 / dir[axis];
            let mut t0 = (bmin[axis] - orig[axis]) * inv_d;
            let mut t1 = (bmax[axis] - orig[axis]) * inv_d;
            let mut sign = -1.0;
            if t0 > t1 {
                std::mem::swap(&mut t0, &mut t1);
                sign = 1.0;
            }
            if t0 > tmin {
                tmin = t0;
                hit_axis = axis;
                hit_sign = sign;
            }
            if t1 < tmax {
                tmax = t1;
            }
            if tmax <= tmin {
                return None;
            }
        }

        if tmin < t_min || tmin > t_max {
            return None;
        }

        let point = ray.at(tmin);
        let size = self.max - self.min;
        let normal;
        let face;
        let u;
        let v;

        match hit_axis {
            0 => {
                normal = Vec3::new(hit_sign, 0.0, 0.0);
                face = if hit_sign > 0.0 { Face::East } else { Face::West };
                u = ((point.z - self.min.z) / size.z).clamp(0.0, 1.0);
                v = ((point.y - self.min.y) / size.y).clamp(0.0, 1.0);
            }
            1 => {
                normal = Vec3::new(0.0, hit_sign, 0.0);
                face = if hit_sign > 0.0 { Face::Top } else { Face::Bottom };
                u = ((point.x - self.min.x) / size.x).clamp(0.0, 1.0);
                v = ((point.z - self.min.z) / size.z).clamp(0.0, 1.0);
            }
            _ => {
                normal = Vec3::new(0.0, 0.0, hit_sign);
                face = if hit_sign > 0.0 { Face::South } else { Face::North };
                u = ((point.x - self.min.x) / size.x).clamp(0.0, 1.0);
                v = ((point.y - self.min.y) / size.y).clamp(0.0, 1.0);
            }
        }

        Some(Hit { t: tmin, point, normal, face, u, v, material: self.material })
    }
}
