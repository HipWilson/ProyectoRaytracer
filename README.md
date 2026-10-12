# Diorama Raytracer

Raytracer escrito desde cero en Rust que renderiza un diorama estilo Minecraft hecho de cubos texturizados.
Todo el trazado de rayos se calcula en la CPU, sin librerías de raytracing ni carga de imágenes.
La única dependencia es `raylib`, usada solo para abrir la ventana y mostrar la imagen ya calculada.

## Video
<img width="948" height="642" alt="Graficas 2" src="https://github.com/user-attachments/assets/de73335b-6296-4662-85be-bce4086a1c8d" />

## Cómo correrlo

```bash
cargo run --release
```

Para generar los cuadros del video sin abrir ventana:

```bash
cargo run --release --example orbit
ffmpeg -framerate 24 -i frames/frame_%04d.bmp -c:v libx264 -pix_fmt yuv420p diorama.mp4
```

## Controles

| Control | Acción |
|---|---|
| Flechas / arrastrar con el mouse | Rotar la cámara |
| W / S / rueda del mouse | Acercar / alejar |
| R | Órbita automática |
| T | Ciclo automático día / noche |
| N | Saltar entre día y noche |
| P | Captura de pantalla |

Si la computadora es lenta, bajar `RENDER_W` y `RENDER_H` en `src/main.rs`.

## Características implementadas

- **Escena:** terreno de 16x16, casa con puerta y ventanas, estanque con muelle, foso de lava, camino, antorchas, vitrina de cristal con oro y bosque.
- **Rotación y zoom de cámara:** flechas, mouse, rueda y órbita automática.
- **Materiales:** 12 (pasto, tierra, arena, piedra, madera, tronco, vidrio, agua, lava, hojas, oro y glowstone), cada uno con su propia textura procedural y sus parámetros de albedo, especular, transparencia y reflectividad.
- **Refracción:** ley de Snell en el vidrio (ventanas y vitrina) y en el agua, con absorción según la profundidad.
- **Reflexión:** espejo metálico en el oro y reflejos de Fresnel en vidrio y agua.
- **Mapas de normales:** ladrillos de piedra, tablones de madera, arena y ondas de agua animadas.
- **Material emisivo:** lava animada y glowstone, que además funcionan como luces puntuales con sombras (antorchas, lámpara de la casa, lava).
- **Skybox:** cubemap de 6 caras generado proceduralmente (nubes de día, estrellas y vía láctea de noche), con sol, luna y ciclo día/noche.
- **Terreno procedural:** alturas por ruido fractal en una rejilla de 16x16 columnas, con capas de bloques y biomas por altura (arena, pasto, piedra).
- **Paralelismo y optimización:**
  - Hilos con reparto dinámico de filas mediante un contador atómico.
  - Terreno recorrido con DDA 2D en vez de probar cada cubo.
  - Objetos organizados en un BVH.
  - Rayos de sombra con salida temprana.
  - Agua como un plano en vez de un cubo por columna.
  - Preview adaptativo mientras se mueve la cámara y refinamiento progresivo al quedarse quieta, hasta resolución completa.
  - Suavizado de texturas según la distancia.

## Estructura

```
src/
  vec3.rs  ray.rs  camera.rs  material.rs   -> matemática, rayo, cámara y materiales
  noise.rs    -> ruido value noise 2D/3D
  texture.rs  -> texturas procedurales, parámetros y mapas de normales
  cube.rs     -> intersección rayo-cubo
  bvh.rs      -> árbol de cajas envolventes
  sky.rs      -> skybox, sol, luna y hora del día
  scene.rs    -> terreno, agua, luces y construcción del diorama
  render.rs   -> trazador de rayos y render paralelo
  main.rs     -> ventana, controles y refinamiento progresivo
tests/smoke.rs     -> pruebas automáticas
examples/          -> bench.rs (mide tiempos) y orbit.rs (genera cuadros del video)
```
