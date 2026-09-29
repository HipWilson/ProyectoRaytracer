# Diorama Raytracer

Raytracer escrito **desde cero en Rust** (sin librerias externas de raytracing, graficos 2D,
ni carga de imagenes) que dibuja un pequeno diorama estilo "Minecraft" hecho de cubos
texturizados. La unica dependencia externa es `raylib`, y solo se usa para abrir la ventana
y mostrar en pantalla el resultado ya calculado; **todo el trazado de rayos ocurre en la CPU**.

<!-- Video del diorama, requerido por el enunciado: -->
[Video del diorama](AGREGAR_LINK_DEL_VIDEO_AQUI)

## Como correrlo

```bash
cargo run --release
```

`--release` es importante: sin optimizaciones el render es notablemente mas lento.

### Controles

| Tecla | Accion |
|---|---|
| Flechas izquierda/derecha | Rotar la camara alrededor del diorama |
| Flechas arriba/abajo | Subir/bajar la camara (pitch) |
| W / S | Acercar / alejar (zoom) |
| Rueda del mouse | Acercar / alejar (zoom) |
| P | Guardar una captura de la ventana actual (`captura_diorama.png`) |

El render se recalcula solo cuando la camara realmente se mueve (no en cada frame), para que
la interaccion sea fluida aunque el trazado sea 100% por CPU.

Si su computadora es mas lenta, bajen `RENDER_W` / `RENDER_H` al inicio de `src/main.rs`.
Si quieren una captura mas nitida para el video, suban esos numeros (o usen `cargo run --release --example bench`
para medir cuanto tarda cada resolucion en su maquina antes de decidir).

## Estructura del proyecto

```
src/
  vec3.rs     -> vectores 3D / color (suma, producto punto, cruz, normalizar, reflejar, refractar)
  ray.rs      -> rayo (origen + direccion)
  noise.rs    -> ruido "value noise" hecho a mano, sin librerias externas
  texture.rs  -> las 6 texturas procedurales (pasto, piedra, madera, vidrio, agua, lava)
  material.rs -> junta una textura con sus parametros fisicos (albedo/especular/transparencia/reflectividad)
  cube.rs     -> interseccion rayo-caja (AABB) + deteccion de cara y coordenadas UV
  scene.rs    -> terreno procedural (16x16) + los objetos del diorama (casa, arbol, laguna de lava)
  camera.rs   -> camara orbital (rotacion + zoom)
  render.rs   -> el trazador de rayos en si (sombras, reflexion, refraccion) + render en paralelo
  main.rs     -> ventana de raylib, entrada de teclado/mouse, dibujar la textura resultante
tests/smoke.rs -> prueba automatica de que el render no truena y produce una imagen variada
examples/bench.rs -> mide cuanto tarda el render a distintas resoluciones en su maquina
```

## Que se implemento (mapeo con la rubrica)

- **Cubos texturizados y diorama**: terreno de 16x16 columnas + una casita (piedra/madera/vidrio),
  un arbol y una laguna de lava sobre una base de piedra.
- **Rotacion de camara + acercar/alejar**: flechas y W/S/rueda del mouse (ver tabla de arriba).
- **6 materiales distintos** (el enunciado pide un maximo de 5 que puntuen, se implementaron 6
  para que la escena se vea mas completa): pasto, piedra, madera, vidrio, agua y lava. Cada uno
  tiene su propia textura *procedural* (generada con ruido, no cargada desde un archivo) y sus
  propios valores de albedo, especular, transparencia y reflectividad (`texture.rs::params`).
- **Refraccion**: el vidrio de la ventana de la casa (indice de refraccion 1.5) y el agua de los
  charcos (1.33) refractan la luz que los atraviesa (`render.rs`, usa la ley de Snell via
  `Vec3::refract`).
- **Reflexion**: se aplica tanto en el vidrio como en el agua (mezcladas con la refraccion usando
  la aproximacion de Fresnel/Schlick, para que el angulo de vision afecte cuanto se ve reflejo
  vs. transparencia, como el agua real).
- **Mapa de normales**: la piedra usa una perturbacion de la normal basada en ruido
  (`texture.rs::normal_perturb`) para simular una superficie rugosa sin necesitar mas geometria.
- **Material emisivo**: la lava brilla con su propio color independientemente de la luz de la
  escena (`render.rs`, rama `if let Some(emissive) = params.emissive`).
- **Skybox**: degradado de cielo + sol, calculado por formula en `scene.rs::skybox` (no es una
  imagen cargada de disco).
- **Terreno procedural** (16x16 columnas, con altura generada por ruido fractal en `scene.rs`),
  incluyendo charcos de agua automaticos en las zonas mas bajas.
- **Paralelismo**: `render.rs` reparte las filas de la imagen entre varios hilos nativos de Rust
  (`std::thread`, uno por nucleo disponible via `std::thread::available_parallelism`), sin usar
  ninguna libreria externa de paralelismo.
- **Optimizacion de algoritmos**: el terreno NO se prueba cubo por cubo. Se guarda solo la altura
  de cada columna y se recorre la rejilla con un algoritmo DDA 2D (Amanatides-Woo, el mismo tipo
  de tecnica que usan los juegos voxel para raycasting), que visita unicamente las celdas que el
  rayo realmente cruza en vez de las 256 columnas completas (`scene.rs::TerrainGrid::hit`).
- **No se usa la tarjeta de video para el calculo**: `render.rs` calcula cada pixel en la CPU;
  raylib solo se usa para *mostrar* el resultado ya calculado en una textura.

## Notas para el video / entrega

1. Corran `cargo run --release`, roten y acerquen la camara para mostrar bien los distintos
   materiales (el vidrio de la ventana, el agua reflejando el cielo, la lava brillando, la piedra
   con relieve).
2. Usen la tecla `P` para guardar capturas, o graben la pantalla directamente.
3. Suban el proyecto a GitHub y peguen el link del video arriba, en este mismo README.
