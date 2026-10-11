// En vez de probar el rayo contra TODOS los cubos de la escena
// el arbol descarta de golpe grupos enteros de cubos cuando el rayo ni siquiera toca su caja.
// Pasa de O(n) cubos por rayo a aproximadamente O(log n).
use crate::cube::{Cube, Hit};
use crate::ray::Ray;

struct Node {
    min: [f32; 3],
    max: [f32; 3],
    // Hoja (n > 0): `a` = indice del primer cubo, `n` = cuantos cubos.
    // Nodo interno (n == 0): el hijo izquierdo es el siguiente nodo, `a` = indice del hijo derecho.
    a: u32,
    n: u32,
}

pub struct Bvh {
    nodes: Vec<Node>,
    cubes: Vec<Cube>,
}

const LEAF_SIZE: usize = 2;

fn bounds_of(cubes: &[Cube]) -> ([f32; 3], [f32; 3]) {
    let mut mn = [f32::INFINITY; 3];
    let mut mx = [f32::NEG_INFINITY; 3];
    for c in cubes {
        let cmin = [c.min.x, c.min.y, c.min.z];
        let cmax = [c.max.x, c.max.y, c.max.z];
        for i in 0..3 {
            mn[i] = mn[i].min(cmin[i]);
            mx[i] = mx[i].max(cmax[i]);
        }
    }
    (mn, mx)
}

fn centroid(c: &Cube, axis: usize) -> f32 {
    match axis {
        0 => (c.min.x + c.max.x) * 0.5,
        1 => (c.min.y + c.max.y) * 0.5,
        _ => (c.min.z + c.max.z) * 0.5,
    }
}

fn build(cubes: &mut [Cube], offset: usize, nodes: &mut Vec<Node>) -> usize {
    let idx = nodes.len();
    let (mn, mx) = bounds_of(cubes);
    nodes.push(Node { min: mn, max: mx, a: 0, n: 0 });

    if cubes.len() <= LEAF_SIZE {
        nodes[idx].a = offset as u32;
        nodes[idx].n = cubes.len() as u32;
        return idx;
    }

    // Partimos por el eje mas largo, a la mediana de los centroides.
    let ext = [mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]];
    let axis = if ext[0] >= ext[1] && ext[0] >= ext[2] { 0 } else if ext[1] >= ext[2] { 1 } else { 2 };
    cubes.sort_by(|p, q| centroid(p, axis).partial_cmp(&centroid(q, axis)).unwrap());
    let mid = cubes.len() / 2;
    let (left, right) = cubes.split_at_mut(mid);
    build(left, offset, nodes);
    let right_idx = build(right, offset + mid, nodes);
    nodes[idx].a = right_idx as u32;
    idx
}

#[inline]
fn aabb_hit(o: [f32; 3], inv: [f32; 3], mn: [f32; 3], mx: [f32; 3], t_max: f32) -> bool {
    let mut t0 = 0.0f32;
    let mut t1 = t_max;
    for a in 0..3 {
        let mut ta = (mn[a] - o[a]) * inv[a];
        let mut tb = (mx[a] - o[a]) * inv[a];
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        t0 = t0.max(ta);
        t1 = t1.min(tb);
        if t1 < t0 {
            return false;
        }
    }
    true
}

#[inline]
fn safe_inv(d: f32) -> f32 {
    if d.abs() < 1e-12 { 1e12 } else { 1.0 / d }
}

impl Bvh {
    pub fn new(mut cubes: Vec<Cube>) -> Self {
        let mut nodes = Vec::new();
        if !cubes.is_empty() {
            build(&mut cubes, 0, &mut nodes);
        }
        Bvh { nodes, cubes }
    }

    pub fn len(&self) -> usize {
        self.cubes.len()
    }

    // Cubo mas cercano que golpea el rayo.
    pub fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        if self.nodes.is_empty() {
            return None;
        }
        let o = [ray.origin.x, ray.origin.y, ray.origin.z];
        let inv = [safe_inv(ray.dir.x), safe_inv(ray.dir.y), safe_inv(ray.dir.z)];

        let mut closest = t_max;
        let mut best: Option<Hit> = None;
        let mut stack = [0u32; 64];
        let mut sp = 1;
        stack[0] = 0;

        while sp > 0 {
            sp -= 1;
            let i = stack[sp] as usize;
            let node = &self.nodes[i];
            if !aabb_hit(o, inv, node.min, node.max, closest) {
                continue;
            }
            if node.n > 0 {
                let first = node.a as usize;
                for k in first..first + node.n as usize {
                    if let Some(h) = self.cubes[k].hit(ray, t_min, closest) {
                        closest = h.t;
                        best = Some(h);
                    }
                }
            } else {
                stack[sp] = node.a;
                stack[sp + 1] = (i + 1) as u32;
                sp += 2;
            }
        }
        best
    }

    // Rayo de sombra: basta saber SI algo opaco bloquea (no necesitamos el mas cercano).
    // El vidrio y el agua (transparency >= 0.5) dejan pasar la luz.
    pub fn occluded(&self, ray: &Ray, t_min: f32, t_max: f32) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        let o = [ray.origin.x, ray.origin.y, ray.origin.z];
        let inv = [safe_inv(ray.dir.x), safe_inv(ray.dir.y), safe_inv(ray.dir.z)];

        let mut stack = [0u32; 64];
        let mut sp = 1;
        stack[0] = 0;
        while sp > 0 {
            sp -= 1;
            let i = stack[sp] as usize;
            let node = &self.nodes[i];
            if !aabb_hit(o, inv, node.min, node.max, t_max) {
                continue;
            }
            if node.n > 0 {
                let first = node.a as usize;
                for k in first..first + node.n as usize {
                    let c = &self.cubes[k];
                    if c.material.params.transparency < 0.5 && c.hit(ray, t_min, t_max).is_some() {
                        return true;
                    }
                }
            } else {
                stack[sp] = node.a;
                stack[sp + 1] = (i + 1) as u32;
                sp += 2;
            }
        }
        false
    }

    // Version "fuerza bruta" para los tests (comprobar que el BVH da el mismo resultado).
    pub fn hit_brute_force(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        let mut closest = t_max;
        let mut best = None;
        for c in &self.cubes {
            if let Some(h) = c.hit(ray, t_min, closest) {
                closest = h.t;
                best = Some(h);
            }
        }
        best
    }
}