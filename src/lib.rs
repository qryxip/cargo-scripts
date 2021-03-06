#![warn(rust_2018_idioms)]

use anyhow::{anyhow, bail, ensure, Context as _};
use cargo_metadata::{Package, Resolve, Target};
use if_chain::if_chain;
use ignore::WalkBuilder;
use indexmap::IndexMap;
use itertools::Itertools as _;
use log::{info, warn, Level, LevelFilter, Log, Record};
use once_cell::sync::OnceCell;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use structopt::clap::AppSettings;
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames, IntoStaticStr, VariantNames as _};
use syn::{Lit, Meta, MetaNameValue};
use termcolor::{BufferedStandardStream, Color, ColorSpec, WriteColor};
use ureq::Response;
use url::Url;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Stdout, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::{env, fs, iter};

#[derive(StructOpt, Debug)]
#[structopt(
    author,
    about,
    bin_name("cargo"),
    global_settings(&[AppSettings::DeriveDisplayOrder, AppSettings::UnifiedHelpMessage])
)]
pub enum Opt {
    #[structopt(author, about)]
    Scripts(OptScripts),
}

#[derive(StructOpt, Debug)]
pub enum OptScripts {
    /// Create a new workspace in an existing directory
    #[structopt(author)]
    InitWorkspace(OptScriptsInitWorkspace),
    /// Create a new workspace member from a template
    #[structopt(author)]
    New(OptScriptsNew),
    /// Remove a workspace member
    #[structopt(author)]
    Rm(OptScriptsRm),
    /// Include a package in the workspace
    #[structopt(author)]
    Include(OptScriptsInclude),
    /// Exclude a package from the workspace
    #[structopt(author)]
    Exclude(OptScriptsExclude),
    /// Import a script as a package (in the same format as `cargo-script`)
    #[structopt(author)]
    Import(OptScriptsImport),
    /// Export a package as a script (in the same format as `cargo-script`)
    #[structopt(author)]
    Export(OptScriptsExport),
    /// Gist
    #[structopt(author)]
    Gist(OptScriptsGist),
    /// Modify cargo-scripts.toml
    #[structopt(author)]
    Config(OptScriptsConfig),
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsInitWorkspace {
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// [cargo] Directory
    #[structopt(default_value("."))]
    pub path: PathBuf,
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsNew {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Set the resulting package name, defaults to the directory name
    #[structopt(long, value_name("NAME"))]
    pub name: Option<String>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// [cargo] Directory
    pub path: PathBuf,
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsRm {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// The **name** of the package to remove
    pub package: String,
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsInclude {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// Path to the Cargo package to include
    pub path: String,
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsExclude {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// Path to the Cargo package to exclude
    pub path: String,
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsImport {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// Path to create the package, defaults to `<workspace-root>/<package-name>`
    #[structopt(long)]
    pub path: Option<PathBuf>,
    /// Path to the script
    pub file: Option<PathBuf>,
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsExport {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// The **name** of the package to export
    pub package: String,
}

#[derive(StructOpt, Debug)]
pub enum OptScriptsGist {
    /// Clone a script from Gist
    #[structopt(author)]
    Clone(OptScriptsGistClone),
    /// Pull a script from Gist
    #[structopt(author)]
    Pull(OptScriptsGistPull),
    /// Pull a script to Gist
    #[structopt(author)]
    Push(OptScriptsGistPush),
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsGistClone {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// Path to create the package, defaults to `<workspace-root>/<package-name>`
    #[structopt(long)]
    pub path: Option<PathBuf>,
    /// Gist ID
    pub gist_id: String,
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsGistPull {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// The **name** of the package to export
    pub package: String,
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsGistPush {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// Create a new gist when `gist_ids.<package>` is not set
    #[structopt(short("u"), long)]
    pub set_upstream: bool,
    /// Make the gist private when `--set-upstream` is enabled
    #[structopt(long)]
    pub private: bool,
    /// Set the description of the gist
    #[structopt(long)]
    pub description: Option<String>,
    /// The **name** of the package to push
    pub package: String,
}

#[derive(StructOpt, Debug)]
pub enum OptScriptsConfig {
    /// Set a variable of cargo-scripts.toml
    Set(OptScriptsConfigSet),
    /// Remove a variable from cargo-scripts.toml
    Remove(OptScriptsConfigRemove),
}

#[derive(StructOpt, Debug)]
pub enum OptScriptsConfigSet {
    /// Set `base`
    Base(OptScriptsConfigSetBase),
    /// Set `gist_id.<package>`
    GistId(OptScriptsConfigSetGistId),
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsConfigSetBase {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// Value
    pub path: String,
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsConfigSetGistId {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// Key (Package **name**)
    pub package: String,
    /// Value
    pub gist_id: String,
}

#[derive(StructOpt, Debug)]
pub enum OptScriptsConfigRemove {
    /// Remove `gist_id.<package>`
    GistId(OptScriptsConfigRmGistId),
}

#[derive(StructOpt, Debug)]
pub struct OptScriptsConfigRmGistId {
    /// [cargo] Path to Cargo.toml
    #[structopt(long, value_name("PATH"))]
    pub manifest_path: Option<PathBuf>,
    /// [cargo] Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        possible_values(AnsiColorChoice::VARIANTS),
        default_value("auto")
    )]
    pub color: AnsiColorChoice,
    /// Dry run
    #[structopt(long)]
    pub dry_run: bool,
    /// Key (Package **name**)
    pub package: String,
}

#[derive(Debug)]
pub struct Context<W, I, P> {
    pub cwd: PathBuf,
    pub home_dir: Option<PathBuf>,
    pub stdout: W,
    pub read_input: I,
    pub read_password: P,
    pub init_logger: fn(AnsiColorChoice),
}

impl Context<Stdout, fn() -> io::Result<String>, fn(&str) -> io::Result<String>> {
    pub fn new() -> anyhow::Result<Self> {
        let cwd = env::current_dir()
            .with_context(|| "couldn't get the current directory of the process")?;
        let home_dir = dirs::home_dir();
        let stdout = io::stdout();

        return Ok(Self {
            cwd,
            home_dir,
            stdout,
            read_input,
            read_password,
            init_logger,
        });

        fn read_input() -> io::Result<String> {
            let mut input = "".to_owned();
            io::stdin().read_to_string(&mut input)?;
            Ok(input)
        }

        fn read_password(prompt: &str) -> io::Result<String> {
            rpassword::read_password_from_tty(Some(prompt))
        }

        fn init_logger(color: AnsiColorChoice) {
            const FILTER_LEVEL: LevelFilter = LevelFilter::Info;
            static FILTER_MODULE: &str = module_path!();

            static LOGGER: OnceCell<Logger<BufferedStandardStream>> = OnceCell::new();

            let logger = LOGGER.get_or_init(|| Logger {
                wtr: Arc::new(Mutex::new(BufferedStandardStream::stderr(match color {
                    AnsiColorChoice::Auto => {
                        if should_enable_for_stderr() {
                            termcolor::ColorChoice::AlwaysAnsi
                        } else {
                            termcolor::ColorChoice::Never
                        }
                    }
                    AnsiColorChoice::Always => termcolor::ColorChoice::AlwaysAnsi,
                    AnsiColorChoice::Never => termcolor::ColorChoice::Never,
                }))),
            });

            if log::set_logger(logger).is_ok() {
                log::set_max_level(FILTER_LEVEL);
            }

            #[cfg(not(windows))]
            fn should_enable_for_stderr() -> bool {
                atty::is(atty::Stream::Stderr)
                    && env::var("TERM").ok().map_or(false, |v| v != "dumb")
            }

            #[cfg(windows)]
            fn should_enable_for_stderr() -> bool {
                use winapi::um::wincon::ENABLE_VIRTUAL_TERMINAL_PROCESSING;
                use winapi_util::HandleRef;

                use std::ops::Deref;

                let term = env::var("TERM");
                let term = term.as_ref().map(Deref::deref);
                if term == Ok("dumb") || term == Ok("cygwin") {
                    false
                } else if env::var_os("MSYSTEM").is_some() && term.is_ok() {
                    atty::is(atty::Stream::Stderr)
                } else {
                    atty::is(atty::Stream::Stderr)
                        && winapi_util::console::mode(HandleRef::stderr())
                            .ok()
                            .map_or(false, |m| m & ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0)
                }
            }

            struct Logger<W> {
                wtr: Arc<Mutex<W>>,
            }

            impl<W: WriteColor + Sync + Send> Log for Logger<W> {
                fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
                    metadata.target().split("::").next() == Some(FILTER_MODULE)
                }

                fn log(&self, record: &Record<'_>) {
                    if self.enabled(record.metadata()) {
                        let mut wtr = self.wtr.lock().unwrap();
                        let (header_fg, header) = match record.level() {
                            Level::Trace => (Color::Magenta, "trace:"),
                            Level::Debug => (Color::Green, "debug:"),
                            Level::Info => (Color::Cyan, "info:"),
                            Level::Warn => (Color::Yellow, "warn:"),
                            Level::Error => (Color::Red, "error:"),
                        };

                        wtr.set_color(
                            ColorSpec::new()
                                .set_fg(Some(header_fg))
                                .set_reset(false)
                                .set_bold(true),
                        )
                        .unwrap();
                        wtr.write_all(header.as_ref()).unwrap();
                        wtr.reset().unwrap();
                        writeln!(wtr, " {}", record.args()).unwrap();
                        wtr.flush().unwrap();
                    }
                }

                fn flush(&self) {}
            }
        }
    }
}

#[derive(EnumString, EnumVariantNames, IntoStaticStr, Debug, Clone, Copy)]
#[strum(serialize_all = "kebab-case")]
pub enum AnsiColorChoice {
    Auto,
    Always,
    Never,
}

pub fn run<W: Write, I: FnOnce() -> io::Result<String>, P: FnMut(&str) -> io::Result<String>>(
    opt: Opt,
    ctx: Context<W, I, P>,
) -> anyhow::Result<()> {
    match opt {
        Opt::Scripts(OptScripts::InitWorkspace(opt)) => init_workspace(opt, ctx),
        Opt::Scripts(OptScripts::New(opt)) => new(opt, ctx),
        Opt::Scripts(OptScripts::Rm(opt)) => rm(opt, ctx),
        Opt::Scripts(OptScripts::Include(opt)) => include(opt, ctx),
        Opt::Scripts(OptScripts::Exclude(opt)) => exclude(opt, ctx),
        Opt::Scripts(OptScripts::Import(opt)) => import(opt, ctx),
        Opt::Scripts(OptScripts::Export(opt)) => export(opt, ctx),
        Opt::Scripts(OptScripts::Gist(OptScriptsGist::Clone(opt))) => gist_clone(opt, ctx),
        Opt::Scripts(OptScripts::Gist(OptScriptsGist::Pull(opt))) => gist_pull(opt, ctx),
        Opt::Scripts(OptScripts::Gist(OptScriptsGist::Push(opt))) => gist_push(opt, ctx),
        Opt::Scripts(OptScripts::Config(OptScriptsConfig::Set(OptScriptsConfigSet::Base(opt)))) => {
            config_set_base(opt, ctx)
        }
        Opt::Scripts(OptScripts::Config(OptScriptsConfig::Set(OptScriptsConfigSet::GistId(
            opt,
        )))) => config_set_gist_id(opt, ctx),
        Opt::Scripts(OptScripts::Config(OptScriptsConfig::Remove(
            OptScriptsConfigRemove::GistId(opt),
        ))) => config_remove_gist_id(opt, ctx),
    }
}

fn init_workspace(
    opt: OptScriptsInitWorkspace,
    ctx: Context<impl Sized, impl Sized, impl Sized>,
) -> anyhow::Result<()> {
    let OptScriptsInitWorkspace {
        color,
        dry_run,
        path,
    } = opt;

    let Context {
        cwd,
        home_dir,
        init_logger,
        ..
    } = ctx;

    init_logger(color);

    let path = cwd.join(path.strip_prefix(".").unwrap_or(&path));

    write(path.join("Cargo.toml"), CARGO_TOML, dry_run)?;
    CargoScriptsConfig::new(&path.join("cargo-scripts.toml"), home_dir.as_deref())?
        .store(dry_run)?;

    let program = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let args = vec![
        OsString::from("new"),
        OsString::from("--vcs"),
        OsString::from("none"),
        path.join("template").into(),
    ];
    info_cmd(&program, &args);
    if !dry_run {
        duct::cmd(program, args).run()?;
    }

    if !dry_run {
        let mut cargo_toml = read_toml_edit(path.join("template").join("Cargo.toml"))?;

        let old_package_version = cargo_toml["package"]["version"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        cargo_toml["package"]["version"] = toml_edit::value("0.0.0");
        info!("`package.version`: {:?} → \"0.0.0\"", old_package_version);
        let old_package_publish = cargo_toml["package"]["publish"].clone();
        cargo_toml["package"]["publish"] = toml_edit::value(false);
        info!("`package.publish`: {:?} → false", old_package_publish);

        write(
            path.join("template").join("Cargo.toml"),
            cargo_toml.to_string(),
            false,
        )?;
    }
    return write(
        path.join("template").join("src").join("main.rs"),
        TEMPLATE_SRC_MAIN_RS,
        dry_run,
    );

    static CARGO_TOML: &str = r#"[workspace]
members = ["template"]
exclude = []
"#;

    static TEMPLATE_SRC_MAIN_RS: &str = r#"#!/usr/bin/env run-cargo-script
//! This is a regular crate doc comment, but it also contains a partial
//! Cargo manifest.  Note the use of a *fenced* code block, and the
//! `cargo` "language".
//!
//! ```cargo
//! # Leave blank.
//! ```

fn main() {
    todo!();
}
"#;
}

fn new(opt: OptScriptsNew, ctx: Context<impl Sized, impl Sized, impl Sized>) -> anyhow::Result<()> {
    let OptScriptsNew {
        manifest_path,
        color,
        name,
        dry_run,
        path,
    } = opt;

    let Context {
        cwd, init_logger, ..
    } = ctx;

    init_logger(color);

    let cargo_metadata::Metadata { workspace_root, .. } =
        cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;

    let path = cwd.join(path.strip_prefix(".").unwrap_or(&path));
    let CargoScriptsConfig { base, .. } = CargoScriptsConfig::load(&workspace_root)?;
    let base = Path::new(&base);
    let base = workspace_root.join(base.strip_prefix(".").unwrap_or(base));

    for entry in WalkBuilder::new(&base).hidden(false).build() {
        match entry {
            Ok(entry) => {
                let src = entry.path();
                let dst = path.join(src.strip_prefix(&base)?);
                if !(src.is_dir() || src == base.join("Cargo.toml")) {
                    if let Some(parent) = dst.parent() {
                        if !parent.exists() {
                            create_dir_all(parent, dry_run)?;
                        }
                    }
                    copy(src, &dst, dry_run)?;
                }
            }
            Err(err) => warn!("{}", err),
        }
    }

    let src_manifest_path = base.join("Cargo.toml");
    let mut cargo_toml = read_toml_edit(&src_manifest_path)?;
    let new_package_name = name.as_deref().map(Ok).unwrap_or_else(|| {
        path.file_name()
            .unwrap_or_default()
            .to_str()
            .with_context(|| format!("the file name of `{}` is not valid UTF-8", path.display()))
    })?;
    modify_package_name(&mut cargo_toml, new_package_name)?;

    let dst_manifest_path = path.join("Cargo.toml");
    write(&dst_manifest_path, cargo_toml.to_string(), dry_run)?;

    modify_ws(
        &workspace_root,
        Some(path.strip_prefix(&base).unwrap_or(&path)),
        None,
        None,
        None,
        dry_run,
    )
}

fn rm(opt: OptScriptsRm, ctx: Context<impl Sized, impl Sized, impl Sized>) -> anyhow::Result<()> {
    let OptScriptsRm {
        manifest_path,
        color,
        dry_run,
        package,
    } = opt;

    let Context {
        cwd, init_logger, ..
    } = ctx;

    init_logger(color);

    let metadata = cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;
    let package = metadata.find_package(&package)?;
    let dir = package
        .manifest_path
        .parent()
        .expect("`manifest_path` should end with \"Cargo.toml\"");

    modify_ws(
        &metadata.workspace_root,
        None,
        None,
        Some(dir),
        Some(dir),
        dry_run,
    )?;

    if !dry_run {
        remove_dir_all::remove_dir_all(dir)?;
    }
    info!("Removed {}", dir.display());
    Ok(())
}

fn include(
    opt: OptScriptsInclude,
    ctx: Context<impl Sized, impl Sized, impl Sized>,
) -> anyhow::Result<()> {
    let OptScriptsInclude {
        manifest_path,
        color,
        dry_run,
        path,
    } = opt;

    let Context {
        cwd, init_logger, ..
    } = ctx;

    init_logger(color);

    let cargo_metadata::Metadata { workspace_root, .. } =
        cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;
    let path = cwd.join(path);

    modify_ws(
        &workspace_root,
        Some(&*path),
        None,
        None,
        Some(&*path),
        dry_run,
    )
}

fn exclude(
    opt: OptScriptsExclude,
    ctx: Context<impl Sized, impl Sized, impl Sized>,
) -> anyhow::Result<()> {
    let OptScriptsExclude {
        manifest_path,
        color,
        dry_run,
        path,
    } = opt;

    let Context {
        cwd, init_logger, ..
    } = ctx;

    init_logger(color);

    let cargo_metadata::Metadata { workspace_root, .. } =
        cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;
    let path = cwd.join(path);

    modify_ws(
        &workspace_root,
        None,
        Some(&*path),
        Some(&*path),
        None,
        dry_run,
    )
}

fn import(
    opt: OptScriptsImport,
    ctx: Context<impl Sized, impl FnOnce() -> io::Result<String>, impl Sized>,
) -> anyhow::Result<()> {
    let OptScriptsImport {
        manifest_path,
        color,
        dry_run,
        path,
        file,
    } = opt;

    let Context {
        cwd,
        read_input,
        init_logger,
        ..
    } = ctx;

    init_logger(color);

    let cargo_metadata::Metadata { workspace_root, .. } =
        cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;

    let content = file
        .as_ref()
        .map(read)
        .unwrap_or_else(move || read_input().map_err(Into::into))?;

    import_script(&workspace_root, &content, dry_run, |package_name| {
        cwd.join(path.unwrap_or_else(|| workspace_root.join(package_name)))
    })
    .map(drop)
}

fn export(
    opt: OptScriptsExport,
    mut ctx: Context<impl Write, impl Sized, impl Sized>,
) -> anyhow::Result<()> {
    let OptScriptsExport {
        manifest_path,
        color,
        package,
    } = opt;

    let Context {
        cwd, init_logger, ..
    } = ctx;

    init_logger(color);

    let metadata = cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;
    let (src_path, cargo_toml) = metadata.find_package(&package)?.find_default_bin()?;
    let (code, _) = replace_cargo_lang_code(&read(src_path)?, &cargo_toml, || {
        anyhow!(
            "could not find the `cargo` code block: {}",
            src_path.display(),
        )
    })?;

    ctx.stdout.write_all(code.as_ref())?;
    ctx.stdout.flush().map_err(Into::into)
}

fn gist_clone(
    opt: OptScriptsGistClone,
    ctx: Context<impl Sized, impl Sized, impl Sized>,
) -> anyhow::Result<()> {
    let OptScriptsGistClone {
        manifest_path,
        color,
        dry_run,
        path,
        gist_id,
    } = opt;

    let Context {
        cwd, init_logger, ..
    } = ctx;

    init_logger(color);

    let cargo_metadata::Metadata { workspace_root, .. } =
        cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;

    let mut config = CargoScriptsConfig::load(&workspace_root)?;

    let (script, _) = retrieve_rust_code(&gist_id)?;
    let package_name = import_script(&workspace_root, &script, dry_run, |package_name| {
        cwd.join(path.unwrap_or_else(|| workspace_root.join(package_name)))
    })?;
    let old_gist_id = config.gist_ids.get(&package_name).cloned();
    info!(
        "`gist_ids.{:?}`: {:?} -> {:?}",
        package_name, old_gist_id, gist_id,
    );
    config.gist_ids.insert(package_name, gist_id);
    config.store(dry_run)?;
    Ok(())
}

fn gist_pull(
    opt: OptScriptsGistPull,
    ctx: Context<impl Sized, impl Sized, impl Sized>,
) -> anyhow::Result<()> {
    let OptScriptsGistPull {
        manifest_path,
        color,
        dry_run,
        package,
    } = opt;

    let Context {
        cwd, init_logger, ..
    } = ctx;

    init_logger(color);

    let metadata = cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;
    let package = metadata.find_package(&package)?;

    let CargoScriptsConfig { gist_ids, .. } = CargoScriptsConfig::load(&metadata.workspace_root)?;
    let gist_id = gist_ids
        .get(&package.name)
        .ok_or_else(|| anyhow!("could not find the `gist_id` for {:?}", package.name))?;
    let (pulled_code, _) = retrieve_rust_code(gist_id)?;
    let (pulled_code, pulled_cargo_toml) = replace_cargo_lang_code_with_default(&pulled_code)?;
    let (src_path, prev_cargo_toml) = package.find_default_bin()?;

    for (path, orig, edit) in &[
        (src_path, read(src_path)?, pulled_code),
        (&package.manifest_path, prev_cargo_toml, pulled_cargo_toml),
    ] {
        if orig == edit {
            info!("No changes: {}", path.display());
        } else {
            info!("`{}`:", path.display());
            for diff in diff::lines(orig, edit) {
                let (pref, line) = match diff {
                    diff::Result::Left(l) => ("-", l),
                    diff::Result::Both(l, _) => (" ", l),
                    diff::Result::Right(l) => ("+", l),
                };
                info!("│{}{}", pref, line);
            }
            write(&path, edit, dry_run)?;
        }
    }
    Ok(())
}

fn gist_push(
    opt: OptScriptsGistPush,
    ctx: Context<impl Sized, impl Sized, impl FnMut(&str) -> io::Result<String>>,
) -> anyhow::Result<()> {
    let OptScriptsGistPush {
        manifest_path,
        color,
        dry_run,
        set_upstream,
        private,
        description,
        package,
    } = opt;

    let Context {
        cwd,
        home_dir,
        read_password,
        init_logger,
        ..
    } = ctx;

    init_logger(color);

    let metadata = cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;
    let mut config = CargoScriptsConfig::load(&metadata.workspace_root)?;
    let github_token = CargoScriptsConfig::load(&metadata.workspace_root)?
        .github_token
        .load_or_ask(dry_run, home_dir.as_deref(), read_password)?;

    let (src_path, cargo_toml) = metadata.find_package(&package)?.find_default_bin()?;
    let (local, _) = replace_cargo_lang_code(&read(src_path)?, &cargo_toml, || {
        anyhow!(
            "could not find the `cargo` code block: {}",
            src_path.display(),
        )
    })?;

    let state = if let Some(gist_id) = config.gist_ids.get(&package) {
        let (remote_code, remote_description) = retrieve_rust_code(gist_id)?;
        if remote_code == local {
            State::UpToDate
        } else {
            State::Forward(gist_id, remote_description)
        }
    } else {
        State::NotExist
    };

    return match state {
        State::UpToDate => {
            info!("Up to date");
            Ok(())
        }
        State::Forward(gist_id, remote_description) => {
            let url = "https://api.github.com/gists/"
                .parse::<Url>()
                .unwrap()
                .join(gist_id)?;

            if dry_run {
                info!("[dry-run] PATCH {}", url);
            } else {
                let payload = json!({
                    "description": description.unwrap_or(remote_description),
                    "files": {
                        format!("{}.rs", package): {
                          "content": local
                        }
                    }
                });

                info!("PATCH {}", url);
                let res = ureq::patch(url.as_ref())
                    .set("Authorization", &format!("token {}", github_token))
                    .set("User-Agent", USER_AGENT)
                    .send_json(payload);
                raise_synthetic_error(&res)?;
                info!("{} {}", res.status(), res.status_text());
                ensure!(res.status() == 200, "expected 200");
                serde_json::from_str::<serde_json::Value>(&res.into_string()?)?;

                info!("Updated `{}`", gist_id);
            }
            Ok(())
        }
        State::NotExist => {
            static URL: &str = "https://api.github.com/gists";

            if !set_upstream {
                bail!("to create a new gist, enable `--set-upstream`");
            } else if dry_run {
                info!("[dry-run] POST {}", URL);
                Ok(())
            } else {
                let payload = json!({
                    "files": {
                        format!("{}.rs", package): {
                          "content": local
                        }
                    },
                    "description": description.unwrap_or_default(),
                    "public": !private
                });

                info!("POST {}", URL);
                let res = ureq::post(URL)
                    .set("Authorization", &format!("token {}", github_token))
                    .set("User-Agent", USER_AGENT)
                    .send_json(payload);
                raise_synthetic_error(&res)?;
                ensure!(res.status() == 201, "expected 201");
                let CreateGist { id } = serde_json::from_str(&res.into_string()?)?;

                info!("Created `{}`", id);
                info!(
                    "`gist_ids.{:?}`: {:?} →> {:?}",
                    package,
                    config.gist_ids.get(&package),
                    id,
                );
                config.gist_ids.insert(package, id);
                config.store(false)
            }
        }
    };

    enum State<'a> {
        UpToDate,
        Forward(&'a str, String),
        NotExist,
    }

    #[derive(Deserialize, Debug)]
    struct CreateGist {
        id: String,
    }

    #[derive(Deserialize, Debug)]
    struct Gist {
        files: IndexMap<String, GistFile>,
    }

    #[derive(Deserialize, Debug)]
    struct GistFile {
        filename: String,
        truncated: bool,
        content: String,
    }
}

fn config_set_base(
    opt: OptScriptsConfigSetBase,
    ctx: Context<impl Sized, impl Sized, impl Sized>,
) -> anyhow::Result<()> {
    let OptScriptsConfigSetBase {
        manifest_path,
        color,
        dry_run,
        path,
    } = opt;

    let Context {
        cwd, init_logger, ..
    } = ctx;

    init_logger(color);

    let cargo_metadata::Metadata { workspace_root, .. } =
        cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;
    let mut config = CargoScriptsConfig::load(&workspace_root)?;
    info!("`base`: {:?} → {:?}", config.base, path);
    config.base = path;
    config.store(dry_run)?;
    Ok(())
}

fn config_set_gist_id(
    opt: OptScriptsConfigSetGistId,
    ctx: Context<impl Sized, impl Sized, impl Sized>,
) -> anyhow::Result<()> {
    let OptScriptsConfigSetGistId {
        manifest_path,
        color,
        dry_run,
        package,
        gist_id,
    } = opt;

    let Context {
        cwd, init_logger, ..
    } = ctx;

    init_logger(color);

    let cargo_metadata::Metadata { workspace_root, .. } =
        cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;
    let mut config = CargoScriptsConfig::load(&workspace_root)?;
    info!(
        "`gist_ids.{:?}`: {:?} → {:?}",
        package,
        config.gist_ids.get(&package),
        gist_id,
    );
    config.gist_ids.insert(package, gist_id);
    config.store(dry_run)?;
    Ok(())
}

fn config_remove_gist_id(
    opt: OptScriptsConfigRmGistId,
    ctx: Context<impl Sized, impl Sized, impl Sized>,
) -> anyhow::Result<()> {
    let OptScriptsConfigRmGistId {
        manifest_path,
        color,
        dry_run,
        package,
    } = opt;

    let Context {
        cwd, init_logger, ..
    } = ctx;

    init_logger(color);

    let cargo_metadata::Metadata { workspace_root, .. } =
        cargo_metadata_no_deps_expecting_virtual(manifest_path.as_deref(), color, &cwd)?;
    let mut config = CargoScriptsConfig::load(&workspace_root)?;
    config.gist_ids.remove(&package);
    info!("Removed `gist_ids.{:?}`", package);
    config.store(dry_run)?;
    Ok(())
}

fn cargo_metadata_no_deps_expecting_virtual(
    cli_option_manifest_path: Option<&Path>,
    cli_option_color: AnsiColorChoice,
    cwd: &Path,
) -> anyhow::Result<cargo_metadata::Metadata> {
    let program = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut args = vec![
        "metadata".into(),
        "--no-deps".into(),
        "--format-version".into(),
        "1".into(),
        "--color".into(),
        <&str>::from(cli_option_color).into(),
        "--frozen".into(),
    ];
    if let Some(cli_option_manifest_path) = cli_option_manifest_path {
        args.push(cwd.join(cli_option_manifest_path));
    }

    info_cmd(&program, &args);
    let metadata = duct::cmd(program, args).dir(cwd).read()?;
    let metadata = serde_json::from_str::<cargo_metadata::Metadata>(&metadata)?;

    if metadata
        .resolve
        .as_ref()
        .map_or(false, |Resolve { root, .. }| root.is_some())
    {
        bail!("the target package must be a virtual manifest");
    }
    Ok(metadata)
}

fn info_cmd(program: impl AsRef<OsStr>, args: &[impl AsRef<OsStr>]) {
    info!(
        "Running `{}{}`",
        shell_escape::escape(program.as_ref().to_string_lossy()),
        args.iter().format_with("", |arg, f| f(&format_args!(
            " {}",
            arg.as_ref().to_string_lossy(),
        ))),
    );
}

fn modify_package_name(cargo_toml: &mut toml_edit::Document, name: &str) -> anyhow::Result<()> {
    let old_name = cargo_toml["package"]["name"]
        .as_str()
        .with_context(|| "`package.name` must be a string")?
        .to_owned();

    cargo_toml["package"]["name"] = toml_edit::value(name);
    info!("`package.name`: {:?} → {:?}", old_name, name);
    Ok(())
}

fn modify_ws<'a>(
    workspace_root: &Path,
    add_to_workspace_members: Option<&'a Path>,
    add_to_workspace_exclude: Option<&'a Path>,
    rm_from_workspace_members: Option<&'a Path>,
    rm_from_workspace_exclude: Option<&'a Path>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let manifest_path = workspace_root.join("Cargo.toml");
    let mut cargo_toml = read_toml_edit(&manifest_path)?;

    for (param, add, rm) in &[
        (
            "members",
            add_to_workspace_members,
            rm_from_workspace_members,
        ),
        (
            "exclude",
            add_to_workspace_exclude,
            rm_from_workspace_exclude,
        ),
    ] {
        let relative_to_root = |path: &'a Path| -> _ {
            let path = path.strip_prefix(workspace_root).unwrap_or(path);
            path.to_str()
                .with_context(|| format!("{:?} is not valid UTF-8 path", path))
        };

        let same_paths = |value: &toml_edit::Value, target: &str| -> _ {
            value.as_str().map_or(false, |s| {
                workspace_root.join(s) == workspace_root.join(target)
            })
        };

        let array = cargo_toml["workspace"][param]
            .or_insert(toml_edit::value(toml_edit::Array::default()))
            .as_array_mut()
            .with_context(|| format!("`workspace.{}` must be an array", param))?;
        if let Some(add) = *add {
            let add = relative_to_root(add)?;
            if !dry_run && array.iter().all(|m| !same_paths(m, add)) {
                array.push(add);
            }
            info!("Added to {:?} to `workspace.{}`", add, param);
        }
        if let Some(rm) = rm {
            let rm = relative_to_root(rm)?;
            if !dry_run {
                let i = array.iter().position(|m| same_paths(m, rm));
                if let Some(i) = i {
                    array.remove(i);
                }
            }
            info!("Removed {:?} from `workspace.{}`", rm, param);
        }
    }

    write(&manifest_path, cargo_toml.to_string(), dry_run)?;
    Ok(())
}

fn import_script(
    workspace_root: &Path,
    script: &str,
    dry_run: bool,
    path: impl FnOnce(&str) -> PathBuf,
) -> anyhow::Result<String> {
    let (main_rs, cargo_toml) = replace_cargo_lang_code_with_default(script)?;

    let package_name = toml::from_str::<CargoToml>(&cargo_toml)
        .with_context(|| "failed to parse the manifest")?
        .package
        .name
        .with_context(|| "missing `package.name`")?;
    let path = path(&package_name);

    create_dir_all(&path, dry_run)?;
    write(path.join("Cargo.toml"), cargo_toml, dry_run)?;

    create_dir_all(path.join("src"), dry_run)?;
    write(path.join("src").join("main.rs"), main_rs, dry_run)?;

    modify_ws(&workspace_root, Some(&*path), None, None, None, dry_run)?;
    Ok(package_name)
}

fn replace_cargo_lang_code_with_default(code: &str) -> anyhow::Result<(String, String)> {
    return replace_cargo_lang_code(code, MANIFEST, || {
        anyhow!("could not find the `cargo` code block")
    });

    static MANIFEST: &str = "# Leave blank.";
}

fn replace_cargo_lang_code(
    code: &str,
    with: &str,
    on_not_found: impl FnOnce() -> anyhow::Error,
) -> anyhow::Result<(String, String)> {
    let mut code_lines = code.lines().map(Cow::from).map(Some).collect::<Vec<_>>();

    let syn::File { shebang, attrs, .. } = syn::parse_file(code)?;
    if shebang.is_some() {
        code_lines[0] = None;
    }

    let mut remove = |i: usize, start: _, end: Option<_>| {
        let entry = &mut code_lines[i];
        if let Some(line) = entry {
            let first = &line[..start];
            let second = match end {
                Some(end) if end < line.len() => &line[end..],
                _ => "",
            };
            *line = format!("{}{}", first, second).into();
            if line.is_empty() {
                *entry = None;
            }
        }
    };

    let mut doc = "".to_owned();

    for attr in attrs {
        if_chain! {
            if let Ok(meta) = attr.parse_meta();
            if let Meta::NameValue(MetaNameValue { path, lit, .. }) = meta;
            if path.get_ident().map_or(false, |i| i == "doc");
            if let Lit::Str(lit_str) = lit;
            then {
                doc += lit_str.value().trim_start_matches(' ');
                doc += "\n";

                for tt in attr.tokens {
                    let (start, end) = (tt.span().start(), tt.span().end());
                    if start.line == end.line {
                        remove(start.line - 1, start.column, Some(end.column));
                    } else {
                        remove(start.line - 1, start.column, None);
                        for i in start.line..end.line - 1 {
                            remove(i, 0, None);
                        }
                        remove(end.line - 1, 0, Some(end.column));
                    }
                }
            }
        }
    }

    let doc_span = pulldown_cmark::Parser::new_ext(&doc, pulldown_cmark::Options::all())
        .into_offset_iter()
        .fold(State::None, |mut state, (event, span)| {
            match &state {
                State::None => {
                    if let pulldown_cmark::Event::Start(pulldown_cmark::Tag::CodeBlock(
                        pulldown_cmark::CodeBlockKind::Fenced(kind),
                    )) = event
                    {
                        if &*kind == "cargo" {
                            state = State::Start;
                        }
                    }
                }
                State::Start => {
                    if let pulldown_cmark::Event::Text(_) = event {
                        state = State::Text(span);
                    }
                }
                State::Text(span) => {
                    if let pulldown_cmark::Event::End(pulldown_cmark::Tag::CodeBlock(
                        pulldown_cmark::CodeBlockKind::Fenced(kind),
                    )) = event
                    {
                        if &*kind == "cargo" {
                            state = State::End(span.clone());
                        }
                    }
                }
                State::End(_) => {}
            }
            state
        })
        .end()
        .with_context(on_not_found)?;

    let with = if with.is_empty() || with.ends_with('\n') {
        with.to_owned()
    } else {
        format!("{}\n", with)
    };

    let converted_doc = format!("{}{}{}", &doc[..doc_span.start], with, &doc[doc_span.end..]);

    let converted_code = shebang
        .map(Into::into)
        .into_iter()
        .chain(converted_doc.lines().map(|line| {
            if line.is_empty() {
                "//!".into()
            } else {
                format!("//! {}", line).into()
            }
        }))
        .chain(code_lines.into_iter().flatten())
        .interleave_shortest(iter::repeat("\n".into()))
        .join("");

    return Ok((converted_code, doc[doc_span].to_owned()));

    #[derive(Debug)]
    enum State {
        None,
        Start,
        Text(Range<usize>),
        End(Range<usize>),
    }

    impl State {
        fn end(self) -> Option<Range<usize>> {
            match self {
                Self::End(span) => Some(span),
                _ => None,
            }
        }
    }
}

fn retrieve_rust_code(gist_id: &str) -> anyhow::Result<(String, String)> {
    let url = "https://api.github.com/gists/"
        .parse::<Url>()
        .unwrap()
        .join(&gist_id)?;

    info!("GET: {}", url);
    let res = ureq::get(url.as_ref()).set("User-Agent", USER_AGENT).call();
    raise_synthetic_error(&res)?;
    info!("{} {}", res.status(), res.status_text());
    ensure!(res.status() == 200, "expected 200");

    let Gist { files, description } = serde_json::from_str(&res.into_string()?)?;

    let file = files
        .values()
        .filter(|GistFile { filename, .. }| {
            [Some("rs".as_ref()), Some("crs".as_ref())].contains(&Path::new(&filename).extension())
        })
        .exactly_one()
        .map_err(|err| {
            let mut err = err.peekable();
            if err.peek().is_some() {
                anyhow!(
                    "multiple Rust files: [{}]",
                    err.format_with(", ", |GistFile { filename, .. }, f| f(&filename)),
                )
            } else {
                anyhow!("no Rust files found")
            }
        })?;

    if file.truncated {
        bail!("{} is truncated", file.filename);
    }

    return Ok((file.content.clone(), description));

    #[derive(Deserialize)]
    struct Gist {
        files: IndexMap<String, GistFile>,
        description: String,
    }

    #[derive(Deserialize, Debug)]
    struct GistFile {
        filename: String,
        truncated: bool,
        content: String,
    }
}

static USER_AGENT: &str = "cargo-scripts <https://github.com/qryxip/cargo-scripts>";

fn raise_synthetic_error(res: &Response) -> anyhow::Result<()> {
    if let Some(err) = res.synthetic_error() {
        let mut err = err as &dyn std::error::Error;
        let mut displays = vec![err.to_string()];
        while let Some(source) = err.source() {
            displays.push(source.to_string());
            err = source;
        }
        let mut displays = displays.into_iter().rev();
        let cause = anyhow!("{}", displays.next().unwrap());
        return Err(displays.fold(cause, |err, display| err.context(display)));
    }
    Ok(())
}

fn read(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|err| match err.kind() {
        io::ErrorKind::InvalidData => anyhow!("path at `{}` was not valid utf-8"),
        _ => anyhow::Error::new(err).context(format!("failed to read {}", path.display())),
    })
}

fn read_toml<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> anyhow::Result<(String, T)> {
    let path = path.as_ref();
    let string = read(path)?;
    let value = toml::from_str(&string)
        .with_context(|| format!("failed to parse the TOML file at {}", path.display()))?;
    Ok((string, value))
}

fn read_toml_edit(path: impl AsRef<Path>) -> anyhow::Result<toml_edit::Document> {
    let path = path.as_ref();
    read(path)?
        .parse()
        .with_context(|| format!("failed to parse the TOML file at {}", path.display()))
}

fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>, dry_run: bool) -> anyhow::Result<()> {
    let path = path.as_ref();
    if !dry_run {
        fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    }
    info!(
        "{}Wrote {}",
        if dry_run { "[dry-run] " } else { "" },
        path.display(),
    );
    Ok(())
}

fn copy(src: impl AsRef<Path>, dst: impl AsRef<Path>, dry_run: bool) -> anyhow::Result<()> {
    let (src, dst) = (src.as_ref(), dst.as_ref());
    if !dry_run {
        fs::copy(src, dst).with_context(|| {
            format!("failed to copy `{}` to `{}`", src.display(), dst.display())
        })?;
    }
    info!(
        "{}Copied {} to {}",
        if dry_run { "[dry-run] " } else { "" },
        src.display(),
        dst.display(),
    );
    Ok(())
}

fn create_dir_all(path: impl AsRef<Path>, dry_run: bool) -> anyhow::Result<()> {
    let path = path.as_ref();
    if !dry_run {
        fs::create_dir_all(path)
            .with_context(|| format!("failed to create directory `{}`", path.display()))?;
    }
    Ok(())
}

trait MetadataExt {
    fn find_package(&self, name: &str) -> anyhow::Result<&Package>;
}

impl MetadataExt for cargo_metadata::Metadata {
    fn find_package(&self, name: &str) -> anyhow::Result<&Package> {
        self.packages
            .iter()
            .find(|p| p.name == name)
            .with_context(|| format!("no such package: {:?}", name))
    }
}

trait PakcageExt {
    fn find_default_bin(&self) -> anyhow::Result<(&Path, String)>;
}

impl PakcageExt for Package {
    fn find_default_bin(&self) -> anyhow::Result<(&Path, String)> {
        let (cargo_toml_str, cargo_toml_value) = read_toml::<_, CargoToml>(&self.manifest_path)?;
        let default_run = cargo_toml_value.package.default_run.as_ref();

        let Target { src_path, .. } = self
            .targets
            .iter()
            .filter(|Target { kind, name, .. }| {
                kind.contains(&"bin".to_owned()) && default_run.map_or(true, |d| d == name)
            })
            .exactly_one()
            .map_err(|err| match err.count() {
                0 => anyhow!("no `bin` targets found"),
                _ => anyhow!("could not determine which `bin` target to export"),
            })?;

        Ok((src_path, cargo_toml_str))
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct CargoScriptsConfig {
    base: String,
    github_token: CargoScriptsConfigGithubToken,
    #[serde(default)]
    gist_ids: BTreeMap<String, String>,
    #[serde(skip)]
    path: PathBuf,
}

impl CargoScriptsConfig {
    fn new(path: &Path, home_dir: Option<&Path>) -> anyhow::Result<Self> {
        let github_token = CargoScriptsConfigGithubToken::File {
            path: {
                let path = dirs::data_local_dir()
                    .with_context(|| "local data directory not found")?
                    .join("cargo-scripts")
                    .join("github-token")
                    .into_os_string()
                    .into_string()
                    .map_err(|s| anyhow!("{:?} is not valid UTF-8", s))?;
                let home_dir = shellexpand::tilde_with_context("~", || home_dir);
                if !home_dir.is_empty() && path.starts_with(&*home_dir) {
                    format!("~{}", path.trim_start_matches(&*home_dir))
                } else {
                    path
                }
            },
        };

        Ok(Self {
            base: "./template".to_owned(),
            github_token,
            gist_ids: BTreeMap::new(),
            path: path.to_owned(),
        })
    }

    fn load(workspace_root: &Path) -> anyhow::Result<Self> {
        let path = workspace_root.join("cargo-scripts.toml");
        let (_, this) = read_toml(&path)?;
        Ok(Self { path, ..this })
    }

    fn store(&self, dry_run: bool) -> anyhow::Result<()> {
        write(&self.path, &toml::to_string(self).unwrap(), dry_run)
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "kind")]
enum CargoScriptsConfigGithubToken {
    File { path: String },
}

impl CargoScriptsConfigGithubToken {
    fn load_or_ask(
        &self,
        dry_run: bool,
        home_dir: Option<&Path>,
        mut ask: impl FnMut(&str) -> io::Result<String>,
    ) -> anyhow::Result<String> {
        let Self::File { path } = self;
        let path = shellexpand::tilde_with_context(path, || home_dir);
        if Path::new(&*path).exists() {
            read(&*path)
        } else {
            let token = ask("GitHub token: ")?;
            if let Some(parent) = Path::new(&*path).parent() {
                create_dir_all(parent, dry_run)?;
            }
            write(&*path, &token, dry_run)?;
            Ok(token)
        }
    }
}

#[derive(Deserialize)]
struct CargoToml {
    #[serde(default)]
    package: CargoTomlPackage,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CargoTomlPackage {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    default_run: Option<String>,
}
