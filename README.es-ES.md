

Hola, soy Andrea.

Estoy aprendiendo Rust y pensé que la mejor manera de hacerlo era sumergirme en un proyecto. Un compilador de markdown a HTML estático es un proyecto muy simple que llevo tiempo teniendo en mente, así que aquí está; de hecho, es el que genera [andreadimatteo.com](https://andreadimatteo.com).

Sin dependencias más allá de un puñado de crates, sin framework de JS, sin generador. Escribes archivos `.md` en `posts/`, ejecutas el compilador y obtienes una carpeta `dist/` llena de HTML plano que GitHub Pages sirve tal cual.

Todo lo que está en el alcance de este proyecto, por lo tanto, el compilador de `.md` a `.html` se implementó sin herramientas de IA generativa y con tanta [documentación](https://doc.rust-lang.org/book/) como sea posible; es simplemente un proyecto para aprender Rust en el camino. Sin embargo, como afirmo que escribí este proyecto yo mismo, creo que es necesario aclarar lo siguiente: el archivo *grotescamente grande* `static/vim.js` es un script generado totalmente por IA para implementar los movimientos de vim en las publicaciones, ~no revisé realmente ni una sola línea de ese código :)~. La IA también generó parte de las plantillas HTML y una parte menor de la hoja de estilos. Pensé que estaban simplemente fuera del alcance de este proyecto.

Llevaré un registro de la cobertura actual de [CommonMark](https://spec.commonmark.org/), las tareas principales, cómo se realiza la compilación y cómo puedes crear publicaciones y usarlo por tu cuenta en [esta publicación](https://andreadimatteo.com/md-to-html-compiler.html).

## build

```sh
cargo run --release
```

Esto lee cada `.md` en `posts/`, escribe un archivo HTML en `posts/`, genera un `index.html` y finalmente copia `static/` en `dist/static/`.

## Makefile

Esto es temporal y no debería aplicarse a nadie más que a mí. Es un conjunto de comandos que escribí por simplicidad:

- `make post` => abre un prompt para crear una plantilla de publicación
- `make gifs` => convierte todos los archivos `.mp4` bajo `static/` a un `gif`. Requiere ffmpeg. Si prestas atención, los `.mp4` actualmente están gitignored ~simplemente no he implementado videos aún, quizás los soporte en el futuro~.
- `make link [pwd]` => crea un enlace simbólico a tu carpeta de publicaciones dentro de `posts/`
