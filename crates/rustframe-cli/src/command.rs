use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "rustframe",
    version,
    about = "Build local-first desktop tools with Rust and web frontends",
    propagate_version = true
)]
pub struct Cli {
    /// Use a project other than the nearest rustframe.json.
    #[arg(long, global = true, value_name = "PATH")]
    pub project: Option<PathBuf>,

    /// Print child-process output and diagnostic context.
    #[arg(long, short, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create an independent Vite project.
    New(NewArgs),
    /// Check the Rust and native desktop toolchain.
    Doctor,
    /// Run the frontend dev server and desktop runner together.
    Dev(DevArgs),
    /// Validate the manifest, assets, schema, and generated types.
    Validate(OutputArgs),
    /// Inspect the resolved public project contract.
    Inspect(InspectArgs),
    /// Explain, diff, and enforce the effective capability policy.
    Capabilities(CapabilitiesArgs),
    /// Generate deterministic database TypeScript types.
    Codegen(CodegenArgs),
    /// Build the frontend and native runner.
    Build(BuildArgs),
    /// Build a host-native installer or bundle.
    Package(PackageArgs),
    /// Manage the local SQLite database.
    Db(DbArgs),
    /// Convert a pre-v1 project to manifest schema v1.
    Migrate(MigrateArgs),
    /// Materialize the generated native runner in native/.
    Eject,

    // Pre-v1 aliases kept only so `rustframe migrate` can be adopted incrementally.
    #[command(hide = true)]
    Export,
    #[command(name = "reset-data", hide = true)]
    ResetData,
    #[command(name = "platform-check", hide = true)]
    PlatformCheck(PlatformCheckArgs),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum Template {
    #[default]
    VanillaTs,
    VanillaJs,
    ReactTs,
    VueTs,
    SvelteTs,
}

impl Template {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VanillaTs => "vanilla-ts",
            Self::VanillaJs => "vanilla-js",
            Self::ReactTs => "react-ts",
            Self::VueTs => "vue-ts",
            Self::SvelteTs => "svelte-ts",
        }
    }

    pub fn uses_typescript(self) -> bool {
        !matches!(self, Self::VanillaJs)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum PackageManager {
    #[default]
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Bun => "bun",
        }
    }
}

#[derive(Debug, Args)]
pub struct NewArgs {
    pub name: String,

    #[arg(long, value_enum)]
    pub template: Option<Template>,

    #[arg(long, value_enum)]
    pub package_manager: Option<PackageManager>,

    /// Install frontend dependencies after scaffolding.
    #[arg(long)]
    pub install: bool,
}

#[derive(Debug, Args)]
pub struct DevArgs {
    /// Override frontend.devUrl for this run.
    #[arg(long, value_name = "URL")]
    pub dev_url: Option<String>,
}

#[derive(Debug, Args)]
pub struct OutputArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct InspectArgs {
    /// Emit the stable machine-readable report.
    #[arg(long)]
    pub json: bool,

    /// Inspect local ownership, remote dependencies, data portability, and packaging policy.
    #[arg(long)]
    pub local_first: bool,

    /// Write the selected JSON report to a file as well as standard output.
    #[arg(long, value_name = "PATH", requires = "json")]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CapabilitiesArgs {
    #[command(subcommand)]
    pub command: CapabilitiesCommand,
}

#[derive(Debug, Subcommand)]
pub enum CapabilitiesCommand {
    /// Explain the effective policy for every declared window and machine scope.
    Explain(OutputArgs),
    /// Compare two manifests or policy snapshots.
    Diff {
        old: PathBuf,
        new: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Compare the current project with a reviewed baseline.
    Check {
        /// Fail when the current policy expands beyond the baseline.
        #[arg(long)]
        deny_expansion: bool,
        /// Baseline path, relative to the project when not absolute.
        #[arg(long, default_value = ".rustframe/capabilities-baseline.json")]
        baseline: PathBuf,
        /// Replace the baseline with the current normalized policy.
        #[arg(long, conflicts_with = "deny_expansion")]
        write_baseline: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Args)]
pub struct CodegenArgs {
    /// Check committed output without modifying it.
    #[arg(long)]
    pub check: bool,
}

#[derive(Debug, Args)]
pub struct BuildArgs {
    /// Compile an optimized native runner.
    #[arg(long, default_value_t = true)]
    pub release: bool,
}

#[derive(Debug, Args)]
pub struct PackageArgs {
    #[arg(long)]
    pub verify: bool,

    /// Limit packaging to one or more host-compatible formats.
    #[arg(long = "format", value_enum)]
    pub formats: Vec<PackageFormat>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum PackageFormat {
    App,
    Dmg,
    Nsis,
    Msi,
    #[value(name = "appimage", alias = "app-image")]
    AppImage,
    Deb,
}

impl PackageFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Dmg => "dmg",
            Self::Nsis => "nsis",
            Self::Msi => "msi",
            Self::AppImage => "appimage",
            Self::Deb => "deb",
        }
    }
}

#[derive(Debug, Args)]
pub struct DbArgs {
    #[command(subcommand)]
    pub command: DbCommand,
}

#[derive(Debug, Subcommand)]
pub enum DbCommand {
    Reset,
    Backup { destination: Option<PathBuf> },
    Restore { source: PathBuf },
}

#[derive(Debug, Args)]
pub struct MigrateArgs {
    /// Report changes without writing files.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct PlatformCheckArgs {
    #[arg(long = "target")]
    pub targets: Vec<String>,
}
