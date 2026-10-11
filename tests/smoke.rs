use diorama_raytracer::bvh::Bvh;
use diorama_raytracer::camera::Camera;
use diorama_raytracer::cube::Cube;
use diorama_raytracer::material::Material;
use diorama_raytracer::ray::Ray;
use diorama_raytracer::render::render;
use diorama_raytracer::scene::{build_diorama, Scene};
use diorama_raytracer::sky::{dir_to_face, Environment};
use diorama_raytracer::texture::{Face, TextureKind};
use diorama_raytracer::vec3::Vec3;
use std::sync::OnceLock;

// Construir la escena (incluye hornear el skybox) toma un rato, la compartimos entre los tests.
fn scene() -> &'static Scene {
    static S: OnceLock<Scene> = OnceLock::new();
    S.get_or_init(build_diorama)
}

fn default_cam() -> Camera {
    Camera { target: Vec3::new(8.0, 3.0, 8.0), yaw: 0.9, pitch: 0.4, distance: 23.0, fov: 0.9 }
}

#[test]
fn renders_without_panicking_and_has_varied_colors() {
    let env = Environment::noon();
    let pixels = render(scene(), &default_cam(), &env, 80, 60, 1);
    assert_eq!(pixels.len(), 80 * 60 * 4);
    let mut distinct = std::collections::HashSet::new();
    for chunk in pixels.chunks(4) {
        distinct.insert((chunk[0], chunk[1], chunk[2]));
    }
    assert!(distinct.len() > 50, "se esperaban varios colores distintos, hubo {}", distinct.len());
}

#[test]
fn render_is_deterministic() {
    let env = Environment::noon();
    let a = render(scene(), &default_cam(), &env, 40, 30, 4);
    let b = render(scene(), &default_cam(), &env, 40, 30, 4);
    assert_eq!(a, b, "dos renders iguales deben dar la misma imagen (sin parpadeo)");
}

#[test]
fn camera_orbit_moves_eye() {
    let cam1 = Camera { target: Vec3::zero(), yaw: 0.0, pitch: 0.0, distance: 10.0, fov: 0.9 };
    let cam2 = Camera { target: Vec3::zero(), yaw: 1.0, pitch: 0.3, distance: 10.0, fov: 0.9 };
    let e1 = cam1.eye();
    let e2 = cam2.eye();
    assert!((e1.x - e2.x).abs() > 0.01 || (e1.y - e2.y).abs() > 0.01);
}

// Regresion: el terreno desaparecia cuando la camara estaba fuera de la rejilla 16x16.
#[test]
fn terrain_is_hit_from_far_outside_the_grid() {
    let s = scene();
    let origin = Vec3::new(40.0, 12.0, 40.0);
    for &(tx, tz) in &[(8.5, 8.5), (1.5, 14.5), (14.5, 1.5), (0.5, 0.5)] {
        let target = Vec3::new(tx, 1.0, tz);
        let ray = Ray::new(origin, target - origin);
        assert!(s.terrain.hit(&ray, 0.001, 1000.0).is_some(), "el rayo hacia ({tx}, {tz}) debia pegar en el terreno");
    }
}

#[test]
fn terrain_is_at_least_16x16() {
    assert!(scene().terrain.size >= 16);
}

#[test]
fn ray_starting_inside_cube_exits_through_far_face() {
    let c = Cube::new(Vec3::zero(), Vec3::splat(1.0), Material::new(TextureKind::Glass));
    let ray = Ray::new(Vec3::new(0.5, 0.5, 0.5), Vec3::new(1.0, 0.0, 0.0));
    let h = c.hit(&ray, 0.001, 100.0).expect("debe salir por la cara +x");
    assert!((h.t - 0.5).abs() < 1e-4);
    assert_eq!(h.face, Face::East);
    assert!(h.normal.x > 0.99, "la normal siempre apunta hacia afuera del cubo");
}

#[test]
fn bvh_matches_brute_force() {
    // Muchos cubos pseudo-aleatorios + muchos rayos: el BVH debe dar exactamente lo mismo.
    let mut seed = 12345u32;
    let mut rnd = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (seed >> 8) as f32 / 16_777_216.0
    };
    let mut cubes = Vec::new();
    for _ in 0..200 {
        let p = Vec3::new(rnd() * 20.0, rnd() * 8.0, rnd() * 20.0);
        let s = Vec3::new(0.3 + rnd() * 1.5, 0.3 + rnd() * 1.5, 0.3 + rnd() * 1.5);
        cubes.push(Cube::new(p, p + s, Material::new(TextureKind::Stone)));
    }
    let bvh = Bvh::new(cubes);
    assert_eq!(bvh.len(), 200);
    for _ in 0..2000 {
        let o = Vec3::new(rnd() * 30.0 - 5.0, rnd() * 14.0, rnd() * 30.0 - 5.0);
        let d = Vec3::new(rnd() * 2.0 - 1.0, rnd() * 2.0 - 1.0, rnd() * 2.0 - 1.0);
        let ray = Ray::new(o, d);
        let a = bvh.hit(&ray, 0.001, 1000.0).map(|h| h.t);
        let b = bvh.hit_brute_force(&ray, 0.001, 1000.0).map(|h| h.t);
        match (a, b) {
            (None, None) => {}
            (Some(x), Some(y)) => assert!((x - y).abs() < 1e-4, "t distinto: {x} vs {y}"),
            other => panic!("BVH y fuerza bruta no coinciden: {:?}", other),
        }
    }
}

#[test]
fn skybox_cubemap_faces_and_day_night() {
    // Las 6 caras del cubemap
    assert_eq!(dir_to_face(Vec3::new(1.0, 0.0, 0.0)).0, 0);
    assert_eq!(dir_to_face(Vec3::new(-1.0, 0.0, 0.0)).0, 1);
    assert_eq!(dir_to_face(Vec3::new(0.0, 1.0, 0.0)).0, 2);
    assert_eq!(dir_to_face(Vec3::new(0.0, -1.0, 0.0)).0, 3);
    assert_eq!(dir_to_face(Vec3::new(0.0, 0.0, 1.0)).0, 4);
    assert_eq!(dir_to_face(Vec3::new(0.0, 0.0, -1.0)).0, 5);

    let s = scene();
    let day = Environment::new(0.20, 0.0);
    let night = Environment::new(0.75, 0.0);
    assert!(day.day_amt > 0.9 && night.day_amt < 0.05);
    let up = Vec3::new(0.1, 1.0, 0.1);
    let c_day = s.sky_color(up, &day);
    let c_night = s.sky_color(up, &night);
    assert!(c_day.length() > c_night.length() * 3.0, "el cielo de dia debe ser mucho mas claro que el de noche");
}

#[test]
fn every_material_has_valid_colors_and_params() {
    let kinds = [
        TextureKind::Grass, TextureKind::Dirt, TextureKind::Sand, TextureKind::Stone,
        TextureKind::Wood, TextureKind::Log, TextureKind::Glass, TextureKind::Water,
        TextureKind::Lava, TextureKind::Leaves, TextureKind::Gold, TextureKind::Glowstone,
    ];
    let faces = [Face::Top, Face::Bottom, Face::North, Face::South, Face::East, Face::West];
    for k in kinds {
        let p = k.params();
        assert!((0.0..=1.0).contains(&p.transparency) && (0.0..=1.0).contains(&p.reflectivity));
        for f in faces {
            for i in 0..40 {
                let (u, v) = (i as f32 * 0.37 + 0.1, i as f32 * 0.53 + 0.2);
                let c = k.color_at(u, v, f, 1.0);
                assert!(c.x.is_finite() && c.y.is_finite() && c.z.is_finite(), "{:?} dio un color invalido", k);
                let n = k.normal_perturb(u, v, 1.0);
                assert!(n.x.is_finite() && n.y.is_finite());
            }
        }
    }
}