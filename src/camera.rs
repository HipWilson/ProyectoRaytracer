use crate::ray::Ray;
use crate::vec3::Vec3;

// Camara que orbita alrededor de un punto (el centro del diorama).
// yaw = angulo horizontal, pitch = angulo vertical, distance = zoom (que tan lejos esta el ojo del target).
#[derive(Clone, Copy)]
pub struct Camera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub fov: f32, // campo de vision vertical, en radianes
}

impl Camera {
    pub fn eye(&self) -> Vec3 {
        let x = self.target.x + self.distance * self.pitch.cos() * self.yaw.sin();
        let y = self.target.y + self.distance * self.pitch.sin();
        let z = self.target.z + self.distance * self.pitch.cos() * self.yaw.cos();
        Vec3::new(x, y, z)
    }

    // s, t en el rango -1..1 (coordenadas de pantalla normalizadas, (0,0) = centro de la imagen).
    pub fn get_ray(&self, s: f32, t: f32, aspect: f32) -> Ray {
        let eye = self.eye();
        let forward = (self.target - eye).normalize();
        let world_up = Vec3::new(0.0, 1.0, 0.0);
        let right = forward.cross(world_up).normalize();
        let up = right.cross(forward).normalize();

        let half_h = (self.fov * 0.5).tan();
        let half_w = half_h * aspect;

        let dir = forward + right * (s * half_w) + up * (t * half_h);
        Ray::new(eye, dir)
    }
}
