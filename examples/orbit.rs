// Genera los cuadros de un video del diorama SIN abrir ventana (todo en CPU, sin librerias):
// la camara da una vuelta completa, hace zoom (acerca y aleja) y la hora pasa de dia a noche.
//
//   cargo run --release --example orbit              (120 cuadros, 960x640, 8 rayos/pixel)
//   cargo run --release --example orbit -- 240 1280 720 8
//
// Los cuadros quedan en la carpeta frames/ como .bmp. Para convertirlos en video (con ffmpeg):
//   ffmpeg -framerate 24 -i frames/frame_%04d.bmp -c:v libx264 -pix_fmt yuv420p diorama.mp4
use diorama_raytracer::camera::Camera;
use diorama_raytracer::render::render;
use diorama_raytracer::scene::build_diorama;
use diorama_raytracer::sky::Environment;
use diorama_raytracer::vec3::Vec3;
use std::f32::consts::PI;
use std::fs;
use std::io::Write;
use std::time::Instant;

// BMP de 24 bits sin compresion (se abre en cualquier visor de imagenes).
fn write_bmp(path: &str, rgba: &[u8], w: usize, h: usize) -> std::io::Result<()> {
    let row_bytes = (w * 3 + 3) & !3;
    let data_size = row_bytes * h;
    let mut f = fs::File::create(path)?;
    f.write_all(b"BM")?;
    f.write_all(&((54 + data_size) as u32).to_le_bytes())?;
    f.write_all(&[0, 0, 0, 0])?;
    f.write_all(&54u32.to_le_bytes())?;
    f.write_all(&40u32.to_le_bytes())?;
    f.write_all(&(w as i32).to_le_bytes())?;
    f.write_all(&(h as i32).to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&24u16.to_le_bytes())?;
    f.write_all(&[0u8; 24])?;
    let mut row = vec![0u8; row_bytes];
    for y in (0..h).rev() {
        for x in 0..w {
            let i = (y * w + x) * 4;
            row[x * 3] = rgba[i + 2];
            row[x * 3 + 1] = rgba[i + 1];
            row[x * 3 + 2] = rgba[i];
        }
        f.write_all(&row)?;
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let frames: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(120);
    let w: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(960);
    let h: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(640);
    let spp: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(8);

    fs::create_dir_all("frames").expect("no se pudo crear la carpeta frames/");
    let scene = build_diorama();

    for i in 0..frames {
        let t = i as f32 / frames as f32; // 0..1
        let cam = Camera {
            target: Vec3::new(8.0, 3.0, 8.0),
            yaw: 0.9 + t * 2.0 * PI,                       // una vuelta completa
            pitch: 0.38 + 0.12 * (t * 2.0 * PI).sin(),     // sube y baja un poco
            distance: 22.0 - 8.0 * (t * 2.0 * PI).sin(),   // se acerca y se aleja
            fov: 0.9,
        };
        // de dia -> atardecer -> noche -> amanecer (una vuelta completa de reloj)
        let env = Environment::new(0.15 + t, i as f32 * 0.1);
        let start = Instant::now();
        let px = render(&scene, &cam, &env, w, h, spp);
        let name = format!("frames/frame_{:04}.bmp", i);
        write_bmp(&name, &px, w, h).expect("no se pudo escribir el cuadro");
        println!("cuadro {}/{} -> {} ({:?})", i + 1, frames, name, start.elapsed());
    }
    println!("Listo. Para armar el video: ffmpeg -framerate 24 -i frames/frame_%04d.bmp -c:v libx264 -pix_fmt yuv420p diorama.mp4");
}