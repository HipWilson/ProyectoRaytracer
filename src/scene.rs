use crate::cube::{Cube, Hit};
use crate::material::Material;
use crate::noise::fbm2;
use crate::ray::Ray;
use crate::texture::TextureKind;
use crate::vec3::{Color, Vec3};

// ------------------------------------------------------------------------------------------------
// Terreno procedural en forma de rejilla (como los "chunks" de Minecraft).
// Optimizacion: en vez de guardar un Cube por cada bloque (16x16x6 = hasta 1536 cajas) y probar
// el rayo contra todas ellas una por una, guardamos solo la ALTURA de cada columna y recorremos
// la rejilla en 2D con un algoritmo DDA (Amanatides-Woo). Esto visita unicamente las celdas que el
// rayo realmente atraviesa en el plano X-Z (tipicamente muchas menos que el total), en vez de una
// busqueda lineal sobre todas las columnas.
// ------------------------------------------------------------------------------------------------
pub struct TerrainGrid {
    pub size: usize,
    heights: Vec<f32>, // heights[z * size + x], en bloques (unidades de mundo)
}

impl TerrainGrid {
    pub fn new(size: usize, seed: u32) -> Self {
        let mut heights = vec![0.0; size * size];
        for z in 0..size {
            for x in 0..size {
                let n = fbm2(x as f32 * 0.15, z as f32 * 0.15, seed, 4);
                heights[z * size + x] = (1.0 + n * 5.0).round().max(1.0);
            }
        }
        TerrainGrid { size, heights }
    }

    fn height_at(&self, x: i32, z: i32) -> f32 {
        if x < 0 || z < 0 || x as usize >= self.size || z as usize >= self.size {
            0.0
        } else {
            self.heights[z as usize * self.size + x as usize]
        }
    }

    pub fn height_at_usize(&self, x: usize, z: usize) -> f32 {
        self.heights[z * self.size + x]
    }

    // Genera charcos de agua en las zonas bajas del terreno (altura menor a `water_level`).
    // Se guardan como cubos normales porque son relativamente pocos.
    pub fn water_cubes(&self, water_level: f32) -> Vec<Cube> {
        let mut out = Vec::new();
        for z in 0..self.size {
            for x in 0..self.size {
                let h = self.height_at_usize(x, z);
                if h < water_level {
                    out.push(Cube::new(
                        Vec3::new(x as f32, h, z as f32),
                        Vec3::new(x as f32 + 1.0, water_level, z as f32 + 1.0),
                        Material::new(TextureKind::Water),
                    ));
                }
            }
        }
        out
    }

    // Recorrido DDA 2D sobre la rejilla del terreno.
    pub fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        let mut x = ray.origin.x.floor() as i32;
        let mut z = ray.origin.z.floor() as i32;

        let step_x: i32 = if ray.dir.x > 1e-8 { 1 } else if ray.dir.x < -1e-8 { -1 } else { 0 };
        let step_z: i32 = if ray.dir.z > 1e-8 { 1 } else if ray.dir.z < -1e-8 { -1 } else { 0 };

        let t_delta_x = if ray.dir.x.abs() > 1e-8 { 1.0 / ray.dir.x.abs() } else { f32::INFINITY };
        let t_delta_z = if ray.dir.z.abs() > 1e-8 { 1.0 / ray.dir.z.abs() } else { f32::INFINITY };

        let next_x_boundary = if step_x > 0 { (x + 1) as f32 } else { x as f32 };
        let next_z_boundary = if step_z > 0 { (z + 1) as f32 } else { z as f32 };

        let mut t_max_x = if step_x != 0 { (next_x_boundary - ray.origin.x) / ray.dir.x } else { f32::INFINITY };
        let mut t_max_z = if step_z != 0 { (next_z_boundary - ray.origin.z) / ray.dir.z } else { f32::INFINITY };

        // Cota de seguridad: como mucho recorremos un poco mas que la diagonal de la rejilla.
        let max_steps = (self.size as i32) * 3 + 4;
        let mut out_of_bounds_streak = 0;

        for _ in 0..max_steps {
            if x >= 0 && z >= 0 && (x as usize) < self.size && (z as usize) < self.size {
                out_of_bounds_streak = 0;
                let h = self.height_at(x, z);
                if h > 0.0 {
                    let cell_min = Vec3::new(x as f32, 0.0, z as f32);
                    let cell_max = Vec3::new(x as f32 + 1.0, h, z as f32 + 1.0);
                    let column = Cube::new(cell_min, cell_max, Material::new(TextureKind::Grass));
                    // Como el DDA visita las celdas en orden creciente de distancia a lo largo del
                    // rayo, la primera celda en la que efectivamente golpeamos la caja ya es la mas
                    // cercana: no hace falta seguir comparando contra el resto de la rejilla.
                    if let Some(hit) = column.hit(ray, t_min, t_max) {
                        return Some(hit);
                    }
                }
            } else {
                out_of_bounds_streak += 1;
                if out_of_bounds_streak > 2 {
                    break; // ya nos alejamos definitivamente de la rejilla
                }
            }

            if t_max_x < t_max_z {
                x += step_x;
                t_max_x += t_delta_x;
            } else {
                z += step_z;
                t_max_z += t_delta_z;
            }
        }
        None
    }
}

// ------------------------------------------------------------------------------------------------
// Escena completa: terreno + objetos del diorama (casa, arbol, laguna de lava).
// ------------------------------------------------------------------------------------------------
pub struct Scene {
    pub terrain: TerrainGrid,
    pub objects: Vec<Cube>,
}

impl Scene {
    pub fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        let mut closest = t_max;
        let mut result = None;

        if let Some(h) = self.terrain.hit(ray, t_min, closest) {
            closest = h.t;
            result = Some(h);
        }
        for obj in &self.objects {
            if let Some(h) = obj.hit(ray, t_min, closest) {
                closest = h.t;
                result = Some(h);
            }
        }
        result
    }

    // Skybox procedural: degradado de cielo + un sol simple. No se carga ninguna imagen.
    pub fn skybox(&self, dir: Vec3) -> Color {
        let d = dir.normalize();
        let t = (d.y * 0.5 + 0.5).clamp(0.0, 1.0);
        let horizon = Vec3::new(0.75, 0.85, 0.95);
        let zenith = Vec3::new(0.25, 0.45, 0.85);
        let mut c = Vec3::lerp(horizon, zenith, t);

        let sun_dir = Vec3::new(0.45, 0.85, 0.35).normalize();
        let s = d.dot(sun_dir);
        if s > 0.998 {
            c = Vec3::new(1.0, 0.95, 0.85);
        } else if s > 0.95 {
            let glow = (s - 0.95) / 0.048;
            c = Vec3::lerp(c, Vec3::new(1.0, 0.9, 0.6), glow * 0.7);
        }
        c
    }
}

// Construye el diorama: terreno procedural de 16x16 + una casita + un arbol + una laguna de lava.
pub fn build_diorama() -> Scene {
    let size = 16usize;
    let terrain = TerrainGrid::new(size, 1337);
    let mut objects = terrain.water_cubes(2.5);

    // --- Casa, sobre una plataforma de piedra para no depender de la altura irregular del terreno ---
    let stone = || Material::new(TextureKind::Stone);
    let wood = || Material::new(TextureKind::Wood);
    let glass = || Material::new(TextureKind::Glass);

    // Plataforma / cimiento
    objects.push(Cube::new(Vec3::new(3.0, 0.0, 3.0), Vec3::new(9.0, 4.0, 9.0), stone()));

    let wall_top = 7.0;
    let wall_bottom = 4.0;
    // Pared sur (con puerta simulada por un hueco no es posible sin resta booleana,
    // asi que dejamos una pared solida y ponemos la ventana en la pared este).
    objects.push(Cube::new(Vec3::new(3.0, wall_bottom, 3.0), Vec3::new(9.0, wall_top, 3.5), wood()));
    // Pared norte
    objects.push(Cube::new(Vec3::new(3.0, wall_bottom, 8.5), Vec3::new(9.0, wall_top, 9.0), wood()));
    // Pared oeste
    objects.push(Cube::new(Vec3::new(3.0, wall_bottom, 3.5), Vec3::new(3.5, wall_top, 8.5), stone()));
    // Pared este, partida para dejar una ventana de vidrio en el medio
    objects.push(Cube::new(Vec3::new(8.5, wall_bottom, 3.5), Vec3::new(9.0, wall_top, 5.5), stone()));
    objects.push(Cube::new(Vec3::new(8.5, wall_bottom, 5.5), Vec3::new(9.0, wall_top, 7.0), glass())); // ventana
    objects.push(Cube::new(Vec3::new(8.5, wall_bottom, 7.0), Vec3::new(9.0, wall_top, 8.5), stone()));
    // Techo
    objects.push(Cube::new(Vec3::new(2.5, wall_top, 2.5), Vec3::new(9.5, wall_top + 0.6, 9.5), wood()));

    // --- Arbol ---
    let trunk_x = 12.0;
    let trunk_z = 4.0;
    objects.push(Cube::new(
        Vec3::new(trunk_x, 0.0, trunk_z),
        Vec3::new(trunk_x + 0.6, 3.2, trunk_z + 0.6),
        wood(),
    ));
    let leaves = || Material::new(TextureKind::Grass);
    objects.push(Cube::new(
        Vec3::new(trunk_x - 1.0, 3.0, trunk_z - 1.0),
        Vec3::new(trunk_x + 1.6, 4.2, trunk_z + 1.6),
        leaves(),
    ));
    objects.push(Cube::new(
        Vec3::new(trunk_x - 0.5, 4.2, trunk_z - 0.5),
        Vec3::new(trunk_x + 1.1, 5.1, trunk_z + 1.1),
        leaves(),
    ));

    // --- Laguna de lava (material emisivo) sobre una base de piedra, para que se note bien el brillo ---
    objects.push(Cube::new(Vec3::new(11.5, 0.0, 10.5), Vec3::new(15.0, 2.0, 14.0), stone()));
    objects.push(Cube::new(
        Vec3::new(11.7, 2.0, 10.7),
        Vec3::new(14.8, 2.35, 13.8),
        Material::new(TextureKind::Lava),
    ));

    Scene { terrain, objects }
}
