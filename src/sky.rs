// Skybox + ambiente 

use crate::noise::{fbm2, fbm3, hash3};
use crate::vec3::{Color, Vec3};
use std::f32::consts::PI;

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ------------------------------------------------------------------------------------------------
// Ambiente: todo lo que depende de la hora del dia (direccion/color de la luz, cielo, niebla).
// ------------------------------------------------------------------------------------------------
#[derive(Clone, Copy)]
pub struct Environment {
    pub time_of_day: f32, // 0..1 : 0 = amanecer, 0.25 = mediodia, 0.5 = atardecer, 0.75 = medianoche
    pub anim_time: f32,   // segundos, para animar agua/lava
    pub sun_dir: Vec3,
    pub light_dir: Vec3,   // direccion de la luz principal (sol de dia, luna de noche)
    pub light_color: Color, // color * intensidad de esa luz
    pub day_amt: f32,      // 0 = noche, 1 = dia
    pub sunset: f32,       // 1 cuando el sol esta en el horizonte
    pub ambient: Color,    // luz ambiental del cielo
    pub fog_color: Color,
}

impl Environment {
    pub fn new(time_of_day: f32, anim_time: f32) -> Self {
        let t = time_of_day - time_of_day.floor();
        let a = t * 2.0 * PI;
        let sun_dir = Vec3::new(a.cos() * 0.7, a.sin(), 0.75).normalize();
        let elev = sun_dir.y;

        let day_amt = smooth(-0.12, 0.22, elev);
        let sunset = (1.0 - elev.abs() * 4.0).clamp(0.0, 1.0);

        let (light_dir, light_color) = if elev >= 0.0 {
            let warm = Vec3::lerp(Vec3::new(1.0, 0.55, 0.25), Vec3::new(1.0, 0.96, 0.88), smooth(0.0, 0.4, elev));
            (sun_dir, warm * (0.9 * smooth(-0.03, 0.22, elev)))
        } else {
            (-sun_dir, Vec3::new(0.55, 0.65, 1.0) * (0.22 * smooth(0.0, 0.2, -elev)))
        };

        let mut ambient = Vec3::lerp(Vec3::new(0.05, 0.07, 0.16), Vec3::new(0.28, 0.34, 0.44), day_amt);
        // al amanecer / atardecer la luz ambiental se tine de naranja
        ambient = Vec3::lerp(ambient, Vec3::new(0.42, 0.28, 0.26), sunset * 0.35 * day_amt);

        let day_h = Vec3::new(0.78, 0.88, 0.97);
        let night_h = Vec3::new(0.03, 0.04, 0.09);
        let mut fog_color = Vec3::lerp(night_h, day_h, day_amt);
        fog_color = Vec3::lerp(fog_color, Vec3::new(1.0, 0.62, 0.42) * 0.85, sunset * 0.45);

        Environment { time_of_day: t, anim_time, sun_dir, light_dir, light_color, day_amt, sunset, ambient, fog_color }
    }

    pub fn noon() -> Self {
        Environment::new(0.20, 0.0)
    }
}

// ------------------------------------------------------------------------------------------------
// Cubemap
// ------------------------------------------------------------------------------------------------
pub struct Skybox {
    size: usize,
    day: Vec<Vec<Color>>,   // 6 caras
    night: Vec<Vec<Color>>, // 6 caras
}

// Direccion que corresponde al texel (a, b) en [-1, 1] de una cara del cubo.
fn face_dir(face: usize, a: f32, b: f32) -> Vec3 {
    match face {
        0 => Vec3::new(1.0, -b, -a),
        1 => Vec3::new(-1.0, -b, a),
        2 => Vec3::new(a, 1.0, b),
        3 => Vec3::new(a, -1.0, -b),
        4 => Vec3::new(a, -b, 1.0),
        _ => Vec3::new(-a, -b, -1.0),
    }
    .normalize()
}

// Inversa: dada una direccion, en que cara pega y en que (a, b) en [-1, 1].
pub fn dir_to_face(d: Vec3) -> (usize, f32, f32) {
    let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
    if ax >= ay && ax >= az {
        if d.x > 0.0 { (0, -d.z / ax, -d.y / ax) } else { (1, d.z / ax, -d.y / ax) }
    } else if ay >= az {
        if d.y > 0.0 { (2, d.x / ay, d.z / ay) } else { (3, d.x / ay, -d.z / ay) }
    } else if d.z > 0.0 {
        (4, d.x / az, -d.y / az)
    } else {
        (5, -d.x / az, -d.y / az)
    }
}

fn day_sky(d: Vec3) -> Color {
    let horizon = Vec3::new(0.80, 0.89, 0.97);
    let zenith = Vec3::new(0.16, 0.38, 0.82);
    let mut c = if d.y >= 0.0 {
        Vec3::lerp(horizon, zenith, d.y.powf(0.55))
    } else {
        Vec3::lerp(horizon, Vec3::new(0.50, 0.56, 0.60), (-d.y * 4.0).clamp(0.0, 1.0))
    };
    if d.y > 0.0 {
        // Nubes: ruido fractal proyectado sobre un plano alto (da perspectiva hacia el horizonte).
        let k = 1.0 / (d.y + 0.22);
        let n = fbm2(d.x * k * 1.7 + 13.0, d.z * k * 1.7 - 7.0, 91, 5);
        let cover = smooth(0.50, 0.72, n) * smooth(0.02, 0.22, d.y);
        let shade = Vec3::lerp(Vec3::new(0.78, 0.82, 0.91), Vec3::splat(1.0), smooth(0.5, 0.9, n));
        c = Vec3::lerp(c, shade, cover * 0.92);
    }
    c
}

fn night_sky(d: Vec3, face: usize, i: usize, j: usize) -> Color {
    let horizon = Vec3::new(0.05, 0.07, 0.15);
    let zenith = Vec3::new(0.005, 0.01, 0.045);
    let mut c = if d.y >= 0.0 {
        Vec3::lerp(horizon, zenith, d.y.powf(0.5))
    } else {
        horizon * 0.6
    };
    if d.y > -0.05 {
        // Via lactea: una banda inclinada de ruido
        let band_n = Vec3::new(0.3, 0.8, 0.5).normalize();
        let dist = d.dot(band_n);
        let milky = (-dist * dist * 14.0).exp() * fbm3(d.x * 6.0, d.y * 6.0, d.z * 6.0, 33, 4);
        c = c + Vec3::new(0.10, 0.11, 0.18) * milky * 1.3;
        // Estrellas: un texel de cada ~350 es una estrella de brillo aleatorio
        let h = hash3(face as i32, i as i32, j as i32, 77);
        if h > 0.9972 {
            let b = 0.45 + 0.55 * hash3(face as i32, i as i32, j as i32, 78);
            c = c + Vec3::new(0.9, 0.92, 1.0) * b;
        }
    }
    c
}

impl Skybox {
    pub fn new(size: usize) -> Self {
        let mut day = Vec::with_capacity(6);
        let mut night = Vec::with_capacity(6);
        for face in 0..6 {
            let mut fd = Vec::with_capacity(size * size);
            let mut fnight = Vec::with_capacity(size * size);
            for j in 0..size {
                for i in 0..size {
                    let a = 2.0 * (i as f32 + 0.5) / size as f32 - 1.0;
                    let b = 2.0 * (j as f32 + 0.5) / size as f32 - 1.0;
                    let d = face_dir(face, a, b);
                    fd.push(day_sky(d));
                    fnight.push(night_sky(d, face, i, j));
                }
            }
            day.push(fd);
            night.push(fnight);
        }
        Skybox { size, day, night }
    }

    fn sample(&self, faces: &[Vec<Color>], d: Vec3) -> Color {
        let (f, a, b) = dir_to_face(d);
        let s = self.size as f32;
        let x = ((a * 0.5 + 0.5) * s - 0.5).clamp(0.0, s - 1.0);
        let y = ((b * 0.5 + 0.5) * s - 0.5).clamp(0.0, s - 1.0);
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(self.size - 1);
        let y1 = (y0 + 1).min(self.size - 1);
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let t = &faces[f];
        let c00 = t[y0 * self.size + x0];
        let c10 = t[y0 * self.size + x1];
        let c01 = t[y1 * self.size + x0];
        let c11 = t[y1 * self.size + x1];
        Vec3::lerp(Vec3::lerp(c00, c10, fx), Vec3::lerp(c01, c11, fx), fy)
    }

    // Color del cielo en una direccion, para la hora del dia indicada.
    pub fn color(&self, dir: Vec3, env: &Environment) -> Color {
        let d = dir.normalize();
        let day = self.sample(&self.day, d) * (0.40 + 0.60 * env.day_amt);
        let night = self.sample(&self.night, d);
        let mut c = Vec3::lerp(night, day, env.day_amt);

        // Resplandor naranja del atardecer / amanecer cerca del horizonte, hacia donde esta el sol.
        let sh = Vec3::new(env.sun_dir.x, 0.0, env.sun_dir.z).normalize();
        let dh = Vec3::new(d.x, 0.0, d.z).normalize();
        let toward = dh.dot(sh).max(0.0);
        let low = (1.0 - d.y.clamp(0.0, 1.0)).powi(3);
        c = c + Vec3::new(1.0, 0.45, 0.15) * (env.sunset * toward * toward * low * 0.9);

        // Sol
        let elev = env.sun_dir.y;
        let sun_vis = smooth(-0.05, 0.03, elev);
        if sun_vis > 0.0 {
            let s = d.dot(env.sun_dir);
            let disc = smooth(0.9988, 0.9996, s);
            let halo = s.max(0.0).powf(48.0) * 0.35;
            c = Vec3::lerp(c, Vec3::new(1.0, 0.95, 0.80) * 1.6, disc * sun_vis);
            c = c + Vec3::new(1.0, 0.80, 0.50) * (halo * sun_vis);
        }
        // Luna (justo opuesta al sol)
        let moon_vis = smooth(-0.05, 0.03, -elev);
        if moon_vis > 0.0 {
            let moon_dir = -env.sun_dir;
            let m = d.dot(moon_dir);
            let disc = smooth(0.9992, 0.9997, m);
            if disc > 0.0 {
                let crater = 0.82 + 0.28 * fbm3(d.x * 90.0, d.y * 90.0, d.z * 90.0, 5, 3);
                c = Vec3::lerp(c, Vec3::new(0.92, 0.94, 1.0) * crater, disc * moon_vis);
            }
            c = c + Vec3::new(0.35, 0.40, 0.65) * (m.max(0.0).powf(60.0) * 0.25 * moon_vis);
        }
        c
    }
}