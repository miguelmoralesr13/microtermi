# Microtermi

Aplicación de escritorio en Rust (egui) para gestionar monorepos de microservicios o microfrontends: descubrimiento de proyectos por `package.json`, ejecución de scripts con un clic, integración Git y variables de entorno por ambiente.

## Requisitos

- Rust (toolchain estable)
- Node/npm, yarn o pnpm en el PATH para ejecutar scripts

## Compilar y ejecutar

```bash
cargo run -p microtermi-gui
```

## Construir .exe (release)

Desde la raíz del proyecto:

```bash
cargo build --release -p microtermi-gui
```

El ejecutable queda en: `target\release\microtermi.exe` (Windows) o `target/release/microtermi` (Linux/macOS).

**Llevarte solo el .exe:** puedes copiar únicamente `microtermi.exe` a otra carpeta o a otro PC con Windows; no hace falta llevar el resto del proyecto. La configuración se guarda en `%APPDATA%\microtermi\`. Si en otro equipo sale un error por falta de DLL (p. ej. `vcruntime140.dll`), instala una vez el [Visual C++ Redistributable](https://aka.ms/vs/17/release/vc_redist.x64.exe).

En Windows también puedes ejecutar el script:

```bash
.\build-exe.bat
```

### Visor de reporte de coverage ("Ver aquí")

En la pestaña **Coverage**, el botón **"Ver aquí"** abre el reporte HTML en una ventana propia (no en el navegador). Para compilar la app y el visor y dejar todo listo en una sola vez:

```bash
.\build-all.bat
```

Ese script compila `microtermi-gui`, compila el visor en `tools/coverage-viewer` y copia el .exe del visor a `target\release\` junto a `microtermi.exe`. Si el visor no está en la misma carpeta que microtermi.exe, "Ver aquí" mostrará un mensaje. **"Abrir en navegador"** siempre abre el reporte en el navegador por defecto.

## Uso

1. **Seleccionar carpeta raíz**: el botón "Seleccionar carpeta raíz" abre un diálogo; elige la raíz del monorepo (donde están las carpetas con `package.json`).
2. **Proyectos**: en el panel izquierdo se listan los proyectos detectados. Selecciona uno para ver sus scripts.
3. **Scripts**: cada script del `package.json` tiene un botón "Ejecutar". En Windows se abre una nueva ventana de consola.
4. **Ejecutar todos**: indica el nombre del script (ej. `dev` o `start`) y elige "Paralelo" o en secuencia; luego "Ejecutar todos".
5. **Variables de entorno**: en "Variables de entorno" puedes elegir ambiente (dev/staging/prod), editar variables, añadir/eliminar y "Guardar en disco". Los archivos son `.env.dev`, `.env.staging`, `.env.prod` en la raíz.
6. **Git**: si la carpeta raíz es un repositorio Git, se muestra la rama actual, selector de rama ("Cambiar rama"), archivos modificados, mensaje de commit y botones Commit, Pull y Push.
7. **GitLab**: en la sección "GitLab" puedes indicar la URL (ej. `https://gitlab.com`) y un token de acceso personal (con scope `api`). "Guardar" persiste la configuración. "Listar proyectos" muestra los proyectos a los que tienes acceso. Al elegir uno se listan sus ramas y puedes "Clonar este proyecto" en una carpeta que elijas.

La última carpeta raíz y la configuración de GitLab (URL y token) se guardan en la configuración y se reabren al iniciar (en `%APPDATA%` o `~/.config` según el SO).

## Estructura del workspace

- `crates/microtermi-core`: lógica (descubrimiento, Git, scripts, env).
- `crates/microtermi-gui`: interfaz egui que usa el core.
