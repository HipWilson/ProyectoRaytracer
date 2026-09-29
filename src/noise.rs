// Ruido "value noise" hecho a mano (sin librerias externas), usado para:
//  - generar el terreno procedural (altura de cada columna)
//  - variar un poco el color de las texturas para que no se vean planas
//  - el mapa de normales de la piedra (bump mapping)

// Hash entero determinista: mismo (x, y, seed) siempre da el mismo resultado.
fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h: u32 = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263))
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
