# Mantenibilidad y cómo seguir creciendo

## Estado actual

- **`app.rs`** (~1950 líneas): contiene tipos (`MainTab`, `TerminalSession`, `MicrotermiApp`), toda la lógica de negocio (refresh, terminal, GitLab, etc.) y **todo el UI** en un único `match self.main_tab` dentro de `update()`.
- **`config.rs`**: configuración persistente (config.json). Ya extraído.
- **`ansi.rs`**: parsing de ANSI para la terminal. Ya extraído.

Para **añadir funcionalidad nueva** sin que el archivo se vuelva ingobernable, conviene seguir extrayendo por dominios.

## Cómo añadir funcionalidad de forma mantenible

### 1. Lógica de negocio

- Si la lógica es **reutilizable o compleja**, ponla en **`microtermi-core`** (por ejemplo: llamadas Git, GitLab, scripts).
- Si es **específica de la GUI** (por ejemplo “al hacer clic en X, hacer Y y actualizar Z”), puede seguir en `impl MicrotermiApp` en `app.rs`, pero agrupada en métodos con nombres claros (`run_script_click`, `refresh_project_git`, etc.).

### 2. UI por pestaña

Para no seguir hinchando el `match self.main_tab` en `update()`:

1. **Hacer público lo necesario** para las pestañas:
   - En `app.rs`, poner `pub(crate) enum MainTab { ... }` para poder usarlo desde otros módulos del crate.
2. **Crear un módulo por pestaña**, por ejemplo:
   - `tabs/settings.rs` → `pub fn draw(app: &mut MicrotermiApp, ui: &mut egui::Ui)`
   - `tabs/projects.rs` → `pub fn draw(app: &mut MicrotermiApp, ui: &mut egui::Ui)`
   - y lo mismo para Git, MultiRun, Coverage.
3. **En `app.rs`, en `update()`**, dejar solo el esqueleto:

   ```rust
   match self.main_tab {
       MainTab::Settings => {
           egui::CentralPanel::default().show(ctx, |ui| {
               egui::ScrollArea::both().show(ui, |ui| {
                   tabs::settings::draw(self, ui);
               });
           });
       }
       MainTab::Projects => { ... }
       // etc.
   }
   ```

4. **Mover el contenido actual** de cada rama del `match` al `draw` correspondiente en su módulo. Así cada pestaña vive en un archivo y es más fácil encontrar y cambiar cosas.

### 3. Nuevas pestañas o bloques grandes de UI

- Añade un nuevo variant a `MainTab` y un nuevo archivo `tabs/mi_tab.rs` con su `draw(app, ui)`.
- En `update()` añade una rama que llame a `tabs::mi_tab::draw(self, ui)` dentro del mismo esquema de `CentralPanel` + `ScrollArea` que el resto.

### 4. Tipos compartidos

- Si un tipo solo lo usa la GUI: en `app.rs` o en el módulo de la pestaña donde tenga más sentido.
- Si lo usan varias pestañas o el core: en `microtermi-core` o en un módulo `types.rs` del crate `microtermi-gui`.

## Resumen

- **Sí es mantenible** si sigues extrayendo: **config/ansi ya están fuera**; el siguiente paso natural es **una función `draw` por pestaña** en `tabs/*.rs` y dejar `app.rs` como orquestador.
- Para **añadir funcionalidad**: preferir métodos con nombres claros en `MicrotermiApp`, y UI nueva en su propio `draw` dentro de `tabs/` (o en un nuevo módulo si es un flujo muy grande).
