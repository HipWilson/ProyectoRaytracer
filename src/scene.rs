use crate::bvh::Bvh;
use crate::cube::{Cube, Hit};
use crate::material::Material;
use crate::noise::{fbm2, value_noise2};
use crate::ray::Ray;
use crate::sky::{Environment, Skybox};
use crate::texture::{Face, TextureKind};
use crate::vec3::{Color, Vec3};

// ------------------------------------------------------------------------------------------------
// Terreno procedural en forma de rejilla.
//
// Optimizacion: en vez de guardar un Cube por cada bloque (16x16x6 = ~1500 cajas) y probar el rayo
// contra todas, guardamos solo la ALTURA de cada columna y recorremos la rejilla en 2D con un
// algoritmo DDA (Amanatides-Woo), que visita unicamente las celdas que el rayo realmente cruza.
//
// Cada columna es una pila de "bloques" virtuales: el material de cada capa se decide al golpear
// (pasto/arena arriba, tierra debajo, piedra mas abajo), igual que en Minecraft.
// El agua NO es un cubo por columna sino un plano a `water_level` (una sola interseccion rayo-plano).
// ------------------------------------------------------------------------------------------------
pub struct TerrainGrid {
    pub size: usize,
    heights: Vec<f32>,
    surface: Vec<TextureKind>,
    pub water_level: f32,
    max_height: f32,
}

// Interseccion rayo - caja, solo para recortar el rayo al volumen del terreno. Devuelve (t_in, t_out).
fn clip_box(ray: &Ray, bmin: Vec3, bmax: Vec3) -> Option<(f32, f32)> {
    let o = [ray.origin.x, ray.origin.y, ray.origin.z];
    let d = [ray.dir.x, ray.dir.y, ray.dir.z];
    let mn = [bmin.x, bmin.y, bmin.z];
    let mx = [bmax.x, bmax.y, bmax.z];
    let mut t0 = f32::NEG_INFINITY;
    let mut t1 = f32::INFINITY;
    for a in 0..3 {
        if d[a].abs() < 1e-8 {
            if o[a] < mn[a] || o[a] > mx[a] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d[a];
        let mut ta = (mn[a] - o[a]) * inv;
        let mut tb = (mx[a] - o[a]) * inv;
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        t0 = t0.max(ta);
        t1 = t1.min(tb);
        if t1 < t0 {
            return None;
        }
    }
    Some((t0, t1))
}

impl TerrainGrid {
    pub fn new(size: usize, seed: u32) -> Self {
        let mut heights = vec![0.0; size * size];
        let mut surface = vec![TextureKind::Grass; size * size];
        for z in 0..size {
            for x in 0..size {
                // ruido fractal (4 octavas) -> altura entera de la columna
                let n = fbm2(x as f32 * 0.19 + 3.1, z as f32 * 0.19 + 7.7, seed, 4);
                let mut h = (3.0 + (n - 0.5) * 7.0).round().clamp(1.0, 6.0);
                // El borde del mapa nunca queda bajo el agua (para que el estanque no se "desborde" por un costado).
                if x == 0 || z == 0 || x == size - 1 || z == size - 1 {
                    h = h.max(3.0);
                }
                heights[z * size + x] = h;
                surface[z * size + x] = Self::biome_for(h);
            }
        }
        let mut t = TerrainGrid { size, heights, surface, water_level: 2.5, max_height: 0.0 };
        t.recompute_max();
        t
    }

    // Bioma segun la altura: arena en las partes bajas, pasto en el medio, piedra en las cumbres.
    fn biome_for(h: f32) -> TextureKind {
        if h <= 2.0 {
            TextureKind::Sand
        } else if h <= 4.0 {
            TextureKind::Grass
        } else {
            TextureKind::Stone
        }
    }

    fn recompute_max(&mut self) {
        self.max_height = self.heights.iter().cloned().fold(0.0, f32::max).max(self.water_level) + 0.01;
    }

    // Modifica una columna (se usa para aplanar la casa y excavar el estanque / foso de lava).
    pub fn set_column(&mut self, x: usize, z: usize, h: f32, kind: TextureKind) {
        self.heights[z * self.size + x] = h;
        self.surface[z * self.size + x] = kind;
        self.recompute_max();
    }

    pub fn height_at(&self, x: i32, z: i32) -> f32 {
        if x < 0 || z < 0 || x as usize >= self.size || z as usize >= self.size {
            0.0
        } else {
            self.heights[z as usize * self.size + x as usize]
        }
    }

    pub fn height_at_usize(&self, x: usize, z: usize) -> f32 {
        self.heights[z * self.size + x]
    }

    pub fn surface_at(&self, x: usize, z: usize) -> TextureKind {
        self.surface[z * self.size + x]
    }

    // Material de un bloque concreto de la columna. `depth` = 0 es el bloque superior.
    fn voxel_kind(&self, col: usize, depth: i32, face: Face) -> TextureKind {
        let surf = self.surface[col];
        match face {
            Face::Top => surf,
            Face::Bottom => TextureKind::Dirt,
            _ => match surf {
                TextureKind::Grass => {
                    if depth == 0 { TextureKind::Grass } else if depth <= 2 { TextureKind::Dirt } else { TextureKind::Stone }
                }
                TextureKind::Sand => {
                    if depth <= 1 { TextureKind::Sand } else { TextureKind::Stone }
                }
                other => other,
            },
        }
    }

    // Superficie del agua: un plano a `water_level`, valido solo sobre las columnas mas bajas que el.
    // Es una sola interseccion rayo-plano (en vez de un cubo por cada columna con agua).
    pub fn water_hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        if ray.dir.y.abs() < 1e-8 {
            return None;
        }
        let t = (self.water_level - ray.origin.y) / ray.dir.y;
        if t < t_min || t > t_max {
            return None;
        }
        let p = ray.at(t);
        let x = p.x.floor() as i32;
        let z = p.z.floor() as i32;
        if x < 0 || z < 0 || x as usize >= self.size || z as usize >= self.size {
            return None;
        }
        if self.height_at(x, z) >= self.water_level {
            return None;
        }
        Some(Hit {
            t,
            point: p,
            normal: Vec3::new(0.0, 1.0, 0.0),
            face: Face::Top,
            u: p.x,
            v: p.z,
            material: Material::new(TextureKind::Water),
        })
    }

    // Recorrido DDA 2D sobre la rejilla. IMPORTANTE: primero se recorta el rayo al volumen del terreno
    // para que funcione aunque la camara este lejos, fuera de la rejilla.
    pub fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        let s = self.size as f32;
        let (t_in, t_out) = clip_box(ray, Vec3::new(0.0, 0.0, 0.0), Vec3::new(s, self.max_height, s))?;
        let t_start = t_in.max(t_min);
        let t_end = t_out.min(t_max);
        if t_start > t_end {
            return None;
        }

        let p = ray.at(t_start);
        let size_i = self.size as i32;
        let mut x = (p.x.floor() as i32).clamp(0, size_i - 1);
        let mut z = (p.z.floor() as i32).clamp(0, size_i - 1);

        let step_x: i32 = if ray.dir.x > 1e-8 { 1 } else if ray.dir.x < -1e-8 { -1 } else { 0 };
        let step_z: i32 = if ray.dir.z > 1e-8 { 1 } else if ray.dir.z < -1e-8 { -1 } else { 0 };
        let t_delta_x = if step_x != 0 { 1.0 / ray.dir.x.abs() } else { f32::INFINITY };
        let t_delta_z = if step_z != 0 { 1.0 / ray.dir.z.abs() } else { f32::INFINITY };
        let mut t_max_x = if step_x > 0 {
            t_start + ((x + 1) as f32 - p.x) / ray.dir.x
        } else if step_x < 0 {
            t_start + (x as f32 - p.x) / ray.dir.x
        } else {
            f32::INFINITY
        };
        let mut t_max_z = if step_z > 0 {
            t_start + ((z + 1) as f32 - p.z) / ray.dir.z
        } else if step_z < 0 {
            t_start + (z as f32 - p.z) / ray.dir.z
        } else {
            f32::INFINITY
        };

        let dummy = Material::new(TextureKind::Stone);
        let mut t_cur = t_start;
        loop {
            if x < 0 || z < 0 || x >= size_i || z >= size_i {
                break;
            }
            let t_exit_cell = t_max_x.min(t_max_z);
            let h = self.heights[z as usize * self.size + x as usize];

            // Descartamos rapido las celdas por las que el rayo pasa completamente por encima de la columna.
            let y0 = ray.origin.y + ray.dir.y * t_cur;
            let y1 = ray.origin.y + ray.dir.y * t_exit_cell.min(t_end);
            if h > 0.0 && y0.min(y1) <= h {
                let column = Cube::new(Vec3::new(x as f32, 0.0, z as f32), Vec3::new(x as f32 + 1.0, h, z as f32 + 1.0), dummy);
                if let Some(mut hit) = column.hit(ray, t_min, t_max) {
                    let col = z as usize * self.size + x as usize;
                    let top_layer = h as i32 - 1;
                    let layer = if hit.face == Face::Top {
                        top_layer
                    } else {
                        ((hit.point.y - 1e-4).floor() as i32).clamp(0, top_layer)
                    };
                    let kind = self.voxel_kind(col, top_layer - layer, hit.face);
                    hit.material = Material::new(kind);
                    return Some(hit);
                }
            }

            if t_exit_cell > t_end {
                break;
            }
            t_cur = t_exit_cell;
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
// Luces puntuales (antorchas, lamparas, brillo de la lava). Son los materiales emisivos que de
// verdad iluminan el entorno, con sombras.
// ------------------------------------------------------------------------------------------------
#[derive(Clone, Copy)]
pub struct PointLight {
    pub pos: Vec3,
    pub color: Color,
    pub radius: f32,
}

// ------------------------------------------------------------------------------------------------
// Escena completa: terreno + objetos (en un BVH) + luces + skybox
// ------------------------------------------------------------------------------------------------
pub struct Scene {
    pub terrain: TerrainGrid,
    pub bvh: Bvh,
    pub lights: Vec<PointLight>,
    pub sky: Skybox,
}

impl Scene {
    pub fn new(terrain: TerrainGrid, objects: Vec<Cube>, lights: Vec<PointLight>) -> Self {
        Scene { terrain, bvh: Bvh::new(objects), lights, sky: Skybox::new(256) }
    }

    pub fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        let mut closest = t_max;
        let mut result = None;
        if let Some(h) = self.terrain.hit(ray, t_min, closest) {
            closest = h.t;
            result = Some(h);
        }
        if let Some(h) = self.terrain.water_hit(ray, t_min, closest) {
            closest = h.t;
            result = Some(h);
        }
        if let Some(h) = self.bvh.hit(ray, t_min, closest) {
            result = Some(h);
        }
        result
    }

    // Rayo de sombra: solo los materiales opacos bloquean la luz (el vidrio y el agua la dejan pasar).
    pub fn occluded(&self, ray: &Ray, t_max: f32) -> bool {
        self.bvh.occluded(ray, 0.001, t_max) || self.terrain.hit(ray, 0.001, t_max).is_some()
    }

    pub fn sky_color(&self, dir: Vec3, env: &Environment) -> Color {
        self.sky.color(dir, env)
    }
}

// Un rectangulo en celdas, para dejar libre de arboles la casa, la lava, etc.
struct Zone {
    x0: i32,
    x1: i32,
    z0: i32,
    z1: i32,
}
impl Zone {
    fn contains(&self, x: i32, z: i32) -> bool {
        x >= self.x0 && x < self.x1 && z >= self.z0 && z < self.z1
    }
}

fn cube(min: (f32, f32, f32), max: (f32, f32, f32), m: Material) -> Cube {
    Cube::new(Vec3::new(min.0, min.1, min.2), Vec3::new(max.0, max.1, max.2), m)
}

// Construye el diorama: terreno procedural 16x16 + casa con ventanas de vidrio + estanque con muelle
// + foso de lava + vitrina de cristal con oro + antorchas + bosque disperso.
pub fn build_diorama() -> Scene {
    let size = 16usize;
    let mut terrain = TerrainGrid::new(size, 1337);

    // --- Modificaciones al terreno procedural ---
    // 1) Meseta plana para la casa
    for z in 2..10 {
        for x in 2..10 {
            terrain.set_column(x, z, 4.0, TextureKind::Grass);
        }
    }
    // 2) Estanque circular (arena en el fondo y la orilla; el agua sube hasta water_level)
    let pond = (12.3f32, 4.3f32);
    for z in 0..size {
        for x in 0..size {
            let d = ((x as f32 + 0.5 - pond.0).powi(2) + (z as f32 + 0.5 - pond.1).powi(2)).sqrt();
            if d < 1.7 {
                terrain.set_column(x, z, 1.0, TextureKind::Sand);
            } else if d < 2.7 {
                terrain.set_column(x, z, 2.0, TextureKind::Sand);
            }
        }
    }
    // 3) Foso de lava: borde de piedra de 1 bloque y fondo mas bajo
    for z in 10..15 {
        for x in 10..15 {
            let rim = x == 10 || x == 14 || z == 10 || z == 14;
            if rim {
                terrain.set_column(x, z, 3.0, TextureKind::Stone);
            } else {
                terrain.set_column(x, z, 2.0, TextureKind::Stone);
            }
        }
    }

    let ground = |x: f32, z: f32| -> f32 {
        terrain.height_at_usize((x.floor() as usize).min(size - 1), (z.floor() as usize).min(size - 1))
    };

    let mut objects: Vec<Cube> = Vec::new();
    let mut lights: Vec<PointLight> = Vec::new();

    let stone = || Material::new(TextureKind::Stone);
    let wood = || Material::new(TextureKind::Wood);
    let log = || Material::new(TextureKind::Log);
    let glass = || Material::new(TextureKind::Glass);
    let gold = || Material::new(TextureKind::Gold);
    let glow = || Material::new(TextureKind::Glowstone);
    let roof = || Material::new_tinted(TextureKind::Wood, Vec3::new(0.80, 0.42, 0.36));

    // ------------------------------- CASA (sobre la meseta y=4) -------------------------------
    let y0 = 4.0;
    objects.push(cube((3.0, y0, 3.0), (9.0, y0 + 0.1, 9.0), wood())); // piso
    // Pilares de las esquinas (troncos)
    for &(cx, cz) in &[(3.0, 3.0), (8.4, 3.0), (3.0, 8.4), (8.4, 8.4)] {
        objects.push(cube((cx, y0, cz), (cx + 0.6, 7.0, cz + 0.6), log()));
    }
    // Pared norte y oeste
    objects.push(cube((3.6, y0, 3.0), (8.4, 7.0, 3.5), wood()));
    objects.push(cube((3.0, y0, 3.6), (3.5, 7.0, 8.4), stone()));
    // Pared este (con ventana grande de vidrio)
    objects.push(cube((8.5, y0, 3.6), (9.0, 5.0, 8.4), stone()));
    objects.push(cube((8.5, 6.4, 3.6), (9.0, 7.0, 8.4), stone()));
    objects.push(cube((8.5, 5.0, 3.6), (9.0, 6.4, 4.8), stone()));
    objects.push(cube((8.5, 5.0, 7.2), (9.0, 6.4, 8.4), stone()));
    objects.push(cube((8.5, 5.0, 4.8), (9.0, 6.4, 7.2), glass()));
    // Pared sur (con puerta abierta y ventanita)
    objects.push(cube((3.6, y0, 8.5), (5.5, 7.0, 9.0), wood()));
    objects.push(cube((5.5, 6.0, 8.5), (6.5, 7.0, 9.0), wood())); // dintel
    objects.push(cube((6.5, y0, 8.5), (7.1, 7.0, 9.0), wood()));
    objects.push(cube((8.1, y0, 8.5), (8.4, 7.0, 9.0), wood()));
    objects.push(cube((7.1, y0, 8.5), (8.1, 5.2, 9.0), wood()));
    objects.push(cube((7.1, 6.2, 8.5), (8.1, 7.0, 9.0), wood()));
    objects.push(cube((7.1, 5.2, 8.5), (8.1, 6.2, 9.0), glass()));
    // Techo escalonado
    objects.push(cube((2.5, 7.0, 2.5), (9.5, 7.5, 9.5), roof()));
    objects.push(cube((3.3, 7.5, 3.3), (8.7, 8.0, 8.7), roof()));
    objects.push(cube((4.2, 8.0, 4.2), (7.8, 8.5, 7.8), roof()));
    objects.push(cube((5.1, 8.5, 5.1), (6.9, 9.0, 6.9), roof()));
    // Ornamento de oro en la cima (refleja el cielo)
    objects.push(cube((5.65, 9.0, 5.65), (6.35, 9.7, 6.35), gold()));
    // Chimenea de piedra
    objects.push(cube((7.4, 7.0, 3.7), (8.1, 9.0, 4.4), stone()));
    // Interior: lampara de glowstone colgada y una mesa
    objects.push(cube((5.7, 6.1, 5.7), (6.3, 6.7, 6.3), glow()));
    lights.push(PointLight { pos: Vec3::new(6.0, 5.9, 6.0), color: Vec3::new(1.0, 0.78, 0.45) * 1.3, radius: 6.5 });
    objects.push(cube((4.0, y0 + 0.1, 4.0), (5.4, 4.9, 5.0), wood()));
    objects.push(cube((4.5, 4.9, 4.3), (5.0, 5.4, 4.8), gold()));

    // Antorchas junto a la puerta y a lo largo del camino
    let torch = |objects: &mut Vec<Cube>, lights: &mut Vec<PointLight>, x: f32, base: f32, z: f32| {
        objects.push(cube((x, base, z), (x + 0.16, base + 0.75, z + 0.16), log()));
        objects.push(cube((x - 0.03, base + 0.75, z - 0.03), (x + 0.19, base + 0.97, z + 0.19), glow()));
        lights.push(PointLight { pos: Vec3::new(x + 0.08, base + 1.15, z + 0.08), color: Vec3::new(1.0, 0.6, 0.25) * 1.2, radius: 5.5 });
    };
    torch(&mut objects, &mut lights, 4.9, y0, 9.5);
    torch(&mut objects, &mut lights, 6.95, y0, 9.5);

    // ------------------------------- CAMINO DE PIEDRA -------------------------------
    for &(cx, cz) in &[(5, 10), (5, 11), (5, 12), (5, 13), (6, 13), (7, 13), (8, 13), (9, 13)] {
        let h = terrain.height_at_usize(cx, cz);
        objects.push(cube((cx as f32 + 0.05, h, cz as f32 + 0.05), (cx as f32 + 0.95, h + 0.1, cz as f32 + 0.95), stone()));
    }
    {
        let h = ground(4.0, 12.0);
        torch(&mut objects, &mut lights, 4.3, h, 12.2);
        let h2 = ground(9.0, 12.0);
        torch(&mut objects, &mut lights, 9.4, h2, 12.2);
    }

    // ------------------------------- VITRINA DE CRISTAL -------------------------------
    // Un bloque de vidrio grueso con oro adentro, sobre un pedestal: aqui se aprecia bien la
    // refraccion (el oro se ve deformado y desplazado) y los reflejos.
    {
        let (vx, vz) = (7.3f32, 10.4f32);
        let g = ground(vx, vz);
        objects.push(cube((vx, g, vz), (vx + 1.1, g + 0.6, vz + 1.1), stone()));
        objects.push(cube((vx, g + 0.6, vz), (vx + 1.1, g + 1.7, vz + 1.1), glass()));
        objects.push(cube((vx + 0.3, g + 0.6, vz + 0.3), (vx + 0.8, g + 1.1, vz + 0.8), gold()));
    }

    // ------------------------------- LAVA -------------------------------
    objects.push(cube((11.0, 2.0, 11.0), (14.0, 2.85, 14.0), Material::new(TextureKind::Lava)));
    lights.push(PointLight { pos: Vec3::new(12.5, 3.6, 12.5), color: Vec3::new(1.0, 0.42, 0.1) * 1.5, radius: 7.5 });

    // ------------------------------- MUELLE EN EL ESTANQUE -------------------------------
    objects.push(cube((9.6, 2.62, 4.0), (12.9, 2.76, 4.8), wood()));
    for &(px, pz) in &[(9.8, 4.05), (9.8, 4.6), (12.7, 4.05), (12.7, 4.6)] {
        objects.push(cube((px, 1.0, pz), (px + 0.18, 3.1, pz + 0.18), log()));
    }
    objects.push(cube((12.68, 3.1, 4.03), (12.94, 3.4, 4.29), glow()));
    lights.push(PointLight { pos: Vec3::new(12.8, 3.7, 4.15), color: Vec3::new(1.0, 0.7, 0.35), radius: 5.5 });

    // ------------------------------- BOSQUE DISPERSO -------------------------------
    let house_zone = Zone { x0: 2, x1: 10, z0: 2, z1: 10 };
    let lava_zone = Zone { x0: 10, x1: 15, z0: 10, z1: 15 };
    let leaf_tints = [
        Vec3::new(0.60, 0.90, 0.40),
        Vec3::new(0.80, 0.75, 0.22),
        Vec3::new(0.90, 0.52, 0.16),
        Vec3::new(0.42, 0.78, 0.42),
    ];
    for z in 0..size as i32 {
        for x in 0..size as i32 {
            if house_zone.contains(x, z) || lava_zone.contains(x, z) {
                continue;
            }
            let (xu, zu) = (x as usize, z as usize);
            if terrain.surface_at(xu, zu) != TextureKind::Grass {
                continue; // solo en pasto (no en arena, piedra ni dentro del agua)
            }
            let d_pond = ((x as f32 + 0.5 - pond.0).powi(2) + (z as f32 + 0.5 - pond.1).powi(2)).sqrt();
            if d_pond < 3.4 {
                continue;
            }
            // dejar libre la franja del camino y la vitrina
            if (x == 5 && z >= 10) || (z == 13 && x >= 5 && x <= 9) || (x >= 7 && x <= 8 && z >= 10 && z <= 11) {
                continue;
            }
            let chance = value_noise2(x as f32 * 0.37 + 11.0, z as f32 * 0.37 + 11.0, 999);
            if chance < 0.64 {
                continue;
            }
            let h = terrain.height_at_usize(xu, zu);
            let pick = value_noise2(x as f32 * 1.7, z as f32 * 1.7, 555);
            let tint = leaf_tints[((pick * leaf_tints.len() as f32) as usize).min(leaf_tints.len() - 1)];
            let leaves = || Material::new_tinted(TextureKind::Leaves, tint);

            let tx = x as f32 + 0.2;
            let tz = z as f32 + 0.2;
            objects.push(cube((tx, h, tz), (tx + 0.6, h + 2.4, tz + 0.6), log()));
            objects.push(cube((tx - 0.8, h + 2.0, tz - 0.8), (tx + 1.4, h + 3.2, tz + 1.4), leaves()));
            objects.push(cube((tx - 0.3, h + 3.2, tz - 0.3), (tx + 0.9, h + 4.1, tz + 0.9), leaves()));
        }
    }

    Scene::new(terrain, objects, lights)
}