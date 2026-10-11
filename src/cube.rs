use crate::material::Material;
use crate::ray::Ray;
use crate::texture::Face;
use crate::vec3::Vec3;

#[derive(Clone, Copy)]
pub struct Cube {
    pub min: Vec3,
    pub max: Vec3,
    pub material: Material,
}

// Resultado de una interseccion rayo-cubo.
#[derive(Clone, Copy)]
pub struct Hit {
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3, // siempre apunta hacia AFUERA del cubo
    pub face: Face,
    pub u: f32,
    pub v: f32,
    pub material: Material,
}

impl Cube {
    pub fn new(min: Vec3, max: Vec3, material: Material) -> Self {
        Cube { min, max, material }
    }

    // Metodo de "slabs" (interseccion rayo-caja alineada a los ejes).
    // Calcula la entrada (t_enter) y la salida (t_exit). Si el rayo ya esta DENTRO del cubo
    // (caso del rayo refractado dentro del vidrio) devolvemos la cara de salida con su normal
    // hacia afuera; asi la refraccion puede salir correctamente del bloque.
    pub fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        let orig = [ray.origin.x, ray.origin.y, ray.origin.z];
        let dir = [ray.dir.x, ray.dir.y, ray.dir.z];
        let bmin = [self.min.x, self.min.y, self.min.z];
        let bmax = [self.max.x, self.max.y, self.max.z];

        let mut t_enter = f32::NEG_INFINITY;
        let mut t_exit = f32::INFINITY;
        let mut enter_axis = 0usize;
        let mut enter_sign = 1.0f32;
        let mut exit_axis = 0usize;
        let mut exit_sign = 1.0f32;

        for axis in 0..3 {
            if dir[axis].abs() < 1e-8 {
                if orig[axis] < bmin[axis] || orig[axis] > bmax[axis] {
                    return None;
                }
                continue;
            }
            let inv_d = 1.0 / dir[axis];
            let mut t0 = (bmin[axis] - orig[axis]) * inv_d;
            let mut t1 = (bmax[axis] - orig[axis]) * inv_d;
            // Si dir > 0 entramos por la cara minima (normal -1); si dir < 0, por la maxima (normal +1).
            let mut sign_in = -1.0;
            if inv_d < 0.0 {
                std::mem::swap(&mut t0, &mut t1);
                sign_in = 1.0;
            }
            if t0 > t_enter {
                t_enter = t0;
                enter_axis = axis;
                enter_sign = sign_in;
            }
            if t1 < t_exit {
                t_exit = t1;
                exit_axis = axis;
                exit_sign = -sign_in;
            }
            if t_exit < t_enter {
                return None;
            }
        }

        let (t, axis, sign) = if t_enter >= t_min {
            (t_enter, enter_axis, enter_sign)
        } else if t_exit >= t_min {
            (t_exit, exit_axis, exit_sign)
        } else {
            return None;
        };
        if t > t_max {
            return None;
        }

        let point = ray.at(t);
        let (normal, face, u, v) = match axis {
            0 => (
                Vec3::new(sign, 0.0, 0.0),
                if sign > 0.0 { Face::East } else { Face::West },
                point.z,
                point.y,
            ),
            1 => (
                Vec3::new(0.0, sign, 0.0),
                if sign > 0.0 { Face::Top } else { Face::Bottom },
                point.x,
                point.z,
            ),
            _ => (
                Vec3::new(0.0, 0.0, sign),
                if sign > 0.0 { Face::South } else { Face::North },
                point.x,
                point.y,
            ),
        };

        Some(Hit { t, point, normal, face, u, v, material: self.material })
    }
}