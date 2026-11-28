//! The built-in posts: GRBL, grblHAL, FluidNC, Mach3 / LinuxCNC and Marlin, as post packages embedded in the binary
//! (`builtin/<name>/post.toml`) and run by the same engine as any package. A job picks one with its `post` setting, as before
//! (phase 4a moves the choice to the machine).
use super::Package;
use crate::model::Job;
use std::sync::OnceLock;

/// The built-in dialects (`Job::post` / `Machine.post`). grblHAL and FluidNC write what GRBL gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect { Grbl, GrblHal, FluidNc, Mach, Marlin }

impl Dialect {
    pub const ALL: [Dialect; 5] = [Dialect::Grbl, Dialect::GrblHal, Dialect::FluidNc, Dialect::Mach, Dialect::Marlin];

    /// The dialect of a `Job::post` / `Machine.post` value; anything unknown is GRBL.
    pub fn from_post(post: &str) -> Dialect {
        match post { "grblhal" => Dialect::GrblHal, "fluidnc" => Dialect::FluidNc, "mach" => Dialect::Mach, "marlin" => Dialect::Marlin, _ => Dialect::Grbl }
    }
    /// The package's folder name under `src/post/builtin/`.
    pub fn key(self) -> &'static str {
        match self { Dialect::Grbl => "grbl", Dialect::GrblHal => "grblhal", Dialect::FluidNc => "fluidnc", Dialect::Mach => "mach", Dialect::Marlin => "marlin" }
    }
    /// The package's manifest.
    pub fn source(self) -> &'static str {
        match self {
            Dialect::Grbl => include_str!("builtin/grbl/post.toml"),
            Dialect::GrblHal => include_str!("builtin/grblhal/post.toml"),
            Dialect::FluidNc => include_str!("builtin/fluidnc/post.toml"),
            Dialect::Mach => include_str!("builtin/mach/post.toml"),
            Dialect::Marlin => include_str!("builtin/marlin/post.toml"),
        }
    }
}

/// The built-in post of a dialect, loaded once.
pub fn builtin(d: Dialect) -> &'static Package {
    static POSTS: OnceLock<Vec<Package>> = OnceLock::new();
    let posts = POSTS.get_or_init(|| Dialect::ALL.iter().map(|d| {
        // a built-in that does not load is a bug in this build; the lint test catches it before a release
        Package::load(&format!("builtin:{}/post.toml", d.key()), d.source()).unwrap_or_else(|e| panic!("built-in post: {e}"))
    }).collect());
    &posts[d as usize]
}

/// The post a job is written with (its `post` setting).
pub fn for_job(job: &Job) -> &'static Package { builtin(Dialect::from_post(&job.post)) }
