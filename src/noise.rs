// usado para:
//  - generar el terreno procedural
//  - texturas por texel estilo Minecraft 
//  - el cielo y el mapa de normales

// Hash entero determinista: mismo (x, y, seed) siempre da el mismo resultado.
pub fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h: u32 = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(2246822519));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h % 100000) as f32 / 100000.0
}

// Igual que hash2 pero con 3 coordenadas (se usa para el cielo y las estrellas).
pub fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h: u32 = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263))
        .wrapping_add((z as u32).wrapping_mul(1103515245))
        .wrapping_add(seed.wrapping_mul(2246822519));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h % 100000) as f32 / 100000.0
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

// Ruido interpolado entre una rejilla de puntos aleatorios (0..1).
pub fn value_noise2(x: f32, y: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let fx = smoothstep(x - x0 as f32);
    let fy = smoothstep(y - y0 as f32);

    let h00 = hash2(x0, y0, seed);
    let h10 = hash2(x0 + 1, y0, seed);
    let h01 = hash2(x0, y0 + 1, seed);
    let h11 = hash2(x0 + 1, y0 + 1, seed);

    let a = h00 * (1.0 - fx) + h10 * fx;
    let b = h01 * (1.0 - fx) + h11 * fx;
    a * (1.0 - fy) + b * fy
}

// Version 3D del value noise.
pub fn value_noise3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let z0 = z.floor() as i32;
    let fx = smoothstep(x - x0 as f32);
    let fy = smoothstep(y - y0 as f32);
    let fz = smoothstep(z - z0 as f32);

    let lerp = |a: f32, b: f32, t: f32| a * (1.0 - t) + b * t;
    let c = |dx: i32, dy: i32, dz: i32| hash3(x0 + dx, y0 + dy, z0 + dz, seed);

    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), fx);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), fx);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), fx);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), fx);
    lerp(lerp(x00, x10, fy), lerp(x01, x11, fy), fz)
}

// Suma de varias "octavas" de ruido para que el resultado se vea mas natural (fractal Brownian motion).
pub fn fbm2(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
    let mut total = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut max_amp = 0.0;
    for i in 0..octaves {
        total += value_noise2(x * freq, y * freq, seed.wrapping_add(i)) * amp;
        max_amp += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    total / max_amp
}

pub fn fbm3(x: f32, y: f32, z: f32, seed: u32, octaves: u32) -> f32 {
    let mut total = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut max_amp = 0.0;
    for i in 0..octaves {
        total += value_noise3(x * freq, y * freq, z * freq, seed.wrapping_add(i)) * amp;
        max_amp += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    total / max_amp
}