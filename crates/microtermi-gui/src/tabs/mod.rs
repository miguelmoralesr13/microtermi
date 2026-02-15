//! Una pestaña por archivo: cada uno exporta `draw(app, ctx)`.

pub mod coverage;
pub mod git;
pub mod multi_run;
pub mod projects;
pub mod settings;

pub use coverage::draw as draw_coverage;
pub use git::draw as draw_git;
pub use multi_run::draw as draw_multi_run;
pub use projects::draw as draw_projects;
pub use settings::draw as draw_settings;
