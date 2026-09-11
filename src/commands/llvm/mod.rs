use super::{
    cache_path, dir_inside_cache_folder, download_ungz_untar, download_unxz_untar,
    get_cmake_default_generator, read_shell, search_cmake, set_current_dir_inside_cache_folder,
    spawn_cmake, write_shell,
};
use crate::tasks::Tasks;
use color_eyre::{eyre::ContextCompat, eyre::Report, eyre::WrapErr, Help};
use serde::{Deserialize, Serialize};

pub fn download_url(version: &str) -> (String, String) {
    (
        format!("https://github.com/llvm/llvm-project/archive/refs/tags/llvmorg-{version}.tar.gz"),
        format!("llvmorg-{version}.tar.gz"),
    )
}

/// A supported LLVM release line (18.1 onwards).
pub struct VersionSpec {
    /// Major.minor identifier, e.g. "22.1".
    pub major_minor: &'static str,
    /// Full release version, e.g. "22.1.8".
    pub full: &'static str,
    /// Environment variable exported for llvm-sys, e.g. "LLVM_SYS_220_PREFIX".
    pub env_var: &'static str,
}

/// All LLVM versions newer than 18 that this tool can install.
/// Patches are pinned to the latest known good release of each line.
pub static VERSIONS: &[VersionSpec] = &[
    VersionSpec { major_minor: "18.1", full: "18.1.8",  env_var: "LLVM_SYS_180_PREFIX" },
    VersionSpec { major_minor: "19.1", full: "19.1.7",  env_var: "LLVM_SYS_190_PREFIX" },
    VersionSpec { major_minor: "20.1", full: "20.1.8",  env_var: "LLVM_SYS_200_PREFIX" },
    VersionSpec { major_minor: "21.1", full: "21.1.7",  env_var: "LLVM_SYS_210_PREFIX" },
    VersionSpec { major_minor: "22.1", full: "22.1.8",  env_var: "LLVM_SYS_220_PREFIX" },
    VersionSpec { major_minor: "23.1", full: "23.1.0",  env_var: "LLVM_SYS_230_PREFIX" },
];

/// Resolve a user supplied version string (major, major.minor or full) to a
/// supported [`VersionSpec`].
pub fn lookup_version(input: &str) -> Option<&'static VersionSpec> {
    let input = input.trim();
    VERSIONS.iter().find(|v| {
        v.full == input
            || v.major_minor == input
            || v.major_minor.split('.').next() == Some(input)
    })
}

/// A subproject of the LLVM monorepo that can be enabled in a build.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Component {
    Clang,
    Lld,
    Lldb,
    ClangToolsExtra,
    CompilerRt,
    Polly,
    Flang,
    Bolt,
}

impl Component {
    /// The name LLVM's CMake build expects in `LLVM_ENABLE_PROJECTS`.
    pub fn cmake_name(&self) -> &'static str {
        match self {
            Component::Clang => "clang",
            Component::Lld => "lld",
            Component::Lldb => "lldb",
            Component::ClangToolsExtra => "clang-tools-extra",
            Component::CompilerRt => "compiler-rt",
            Component::Polly => "polly",
            Component::Flang => "flang",
            Component::Bolt => "bolt",
        }
    }

    /// Parse a single component name, accepting a few friendly aliases.
    pub fn parse(s: &str) -> Option<Component> {
        match s.trim().to_ascii_lowercase().as_str() {
            "clang" => Some(Component::Clang),
            "lld" => Some(Component::Lld),
            "lldb" => Some(Component::Lldb),
            "clang-tools-extra" | "extra" | "clang-tools" => Some(Component::ClangToolsExtra),
            "compiler-rt" | "crt" | "compilerrt" => Some(Component::CompilerRt),
            "polly" => Some(Component::Polly),
            "flang" => Some(Component::Flang),
            "bolt" => Some(Component::Bolt),
            _ => None,
        }
    }
}

/// The components built when the user does not pass `--components`.
pub fn default_components() -> Vec<Component> {
    vec![Component::Clang, Component::Lld]
}

/// Parse a comma separated list of components, returning an error listing the
/// valid options when an unknown name is given.
pub fn parse_components(input: &str) -> Result<Vec<Component>, Report> {
    let mut out = vec![];
    for raw in input.split(',') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        match Component::parse(raw) {
            Some(c) => out.push(c),
            None => return Err(color_eyre::eyre::eyre!(
                "unknown component `{raw}`; valid options are: \
                 clang, lld, lldb, clang-tools-extra, compiler-rt, polly, flang, bolt"
            )),
        }
    }
    if out.is_empty() {
        return Err(color_eyre::eyre::eyre!(
            "no components selected; valid options are: \
             clang, lld, lldb, clang-tools-extra, compiler-rt, polly, flang, bolt"
        ));
    }
    Ok(out)
}

/// Record persisted to disk once an install completes, so a later run can resume
/// (or skip) without redoing finished stages.
#[derive(Serialize, Deserialize, Default)]
struct InstallRecord {
    components: Vec<String>,
    targets: String,
}

impl InstallRecord {
    fn matches(&self, components: &[Component], targets: &str) -> bool {
        self.targets == targets
            && components
                .iter()
                .all(|c| self.components.contains(&c.cmake_name().to_string()))
    }
}

/// The outcome of a single install stage.
#[derive(Debug, Clone)]
pub enum StageStatus {
    /// The stage ran and completed, with a short human readable detail.
    Done(String),
    /// The stage was skipped because its output was already present.
    Skipped(String),
}

/// A human readable report of what an install run actually did. Returned by
/// [`install_version`] so the caller can print it.
#[derive(Debug, Default)]
pub struct InstallReport {
    pub version: String,
    pub install_prefix: String,
    pub env_var: String,
    pub env_value: String,
    pub components: Vec<String>,
    pub targets: String,
    pub stages: Vec<(&'static str, StageStatus)>,
}

impl InstallReport {
    fn stage(&mut self, name: &'static str, status: StageStatus) {
        self.stages.push((name, status));
    }

    /// Print a summary of what the install did (and what it skipped).
    pub fn print_summary(&self) {
        let bold = "\x1b[1m";
        let dim = "\x1b[2m";
        let green = "\x1b[32m";
        let yellow = "\x1b[33m";
        let reset = "\x1b[0m";

        // If the install stage was skipped, nothing was actually built this run
        // (we only refreshed the env var).
        let already = self
            .stages
            .iter()
            .any(|(name, s)| *name == "install" && matches!(s, StageStatus::Skipped(_)));

        if already {
            println!(
                "{bold}LLVM {ver}{reset} is already installed at {dim}{prefix}{reset}",
                ver = self.version,
                prefix = self.install_prefix
            );
        } else {
            println!(
                "{bold}Installed LLVM {ver}{reset} → {dim}{prefix}{reset}",
                ver = self.version,
                prefix = self.install_prefix
            );
        }

        println!();
        println!("{dim}Stages:{reset}");
        for (name, status) in &self.stages {
            match status {
                StageStatus::Done(detail) => println!(
                    "  {green}✓{reset} {name:<12} {dim}{detail}{reset}",
                    name = name,
                    detail = detail
                ),
                StageStatus::Skipped(reason) => println!(
                    "  {yellow}•{reset} {name:<12} {dim}skipped ({reason}){reset}",
                    name = name,
                    reason = reason
                ),
            }
        }

        println!();
        println!(
            "{dim}Components:{reset} {}   {dim}Targets:{reset} {}",
            if self.components.is_empty() {
                "(llvm core only)".to_string()
            } else {
                self.components.join(", ")
            },
            self.targets
        );
        if !self.env_var.is_empty() {
            println!(
                "{dim}Env:{reset} {bold}{}{reset}={}",
                self.env_var, self.env_value
            );
        }
        println!(
            "{dim}Re-run `llvmgr env bash` and `eval \"$(llvmgr env bash)\"` to use it.{reset}"
        );
    }
}

fn record_path(version: &str) -> Result<std::path::PathBuf, Report> {
    Ok(dir_inside_cache_folder(version)?.join(".llvmgr_installed"))
}

fn cfg_marker_path(version: &str) -> Result<std::path::PathBuf, Report> {
    Ok(dir_inside_cache_folder(format!("{version}/src/build"))?.join(".llvmgr_cfg"))
}

fn read_record(version: &str) -> Option<InstallRecord> {
    let path = record_path(version).ok()?;
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn write_record(version: &str, record: &InstallRecord) -> Result<(), Report> {
    let path = record_path(version)?;
    let data = serde_json::to_string_pretty(record).wrap_err("serializing install record")?;
    std::fs::write(path, data).wrap_err("writing install record")?;
    Ok(())
}

fn read_cfg_marker(version: &str) -> Option<InstallRecord> {
    let path = cfg_marker_path(version).ok()?;
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn write_cfg_marker(version: &str, record: &InstallRecord) -> Result<(), Report> {
    let path = cfg_marker_path(version)?;
    let data = serde_json::to_string_pretty(record).wrap_err("serializing config marker")?;
    std::fs::write(path, data).wrap_err("writing config marker")?;
    Ok(())
}

fn source_present(version: &str) -> bool {
    cache_path(format!("{version}/src/llvm/CMakeLists.txt"))
        .map(|p| p.exists())
        .unwrap_or(false)
}

fn build_configured(version: &str) -> bool {
    cache_path(format!("{version}/src/build/CMakeCache.txt"))
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Download, compile and install a specific LLVM version.
///
/// `version` is the full release version (e.g. "22.1.8"), `env_var` is the
/// llvm-sys prefix variable to record (e.g. "LLVM_SYS_220_PREFIX").
///
/// The install is resumable: each stage (download/extract, configure, build,
/// install) is skipped when its output is already on disk, and Ninja resumes
/// partial builds incrementally. A `.llvmgr_installed` marker records which
/// components/targets have already been installed.
///
/// Returns an [`InstallReport`] describing what each stage did (or skipped),
/// so the caller can print a summary.
pub async fn install_version(
    version: &str,
    env_var: &str,
    components: &[Component],
    targets: &str,
    reinstall: bool,
) -> Result<InstallReport, Report> {
    let version_root_folder = dir_inside_cache_folder(version)?;
    let llvm_source_code_folder = dir_inside_cache_folder(format!("{version}/src"))?;
    let (source_code_url, source_code_filename) = download_url(version);

    let requested_components: Vec<String> =
        components.iter().map(|c| c.cmake_name().to_string()).collect();

    let mut report = InstallReport {
        version: version.to_string(),
        install_prefix: version_root_folder.display().to_string(),
        env_var: env_var.to_string(),
        components: requested_components.clone(),
        targets: targets.to_string(),
        ..Default::default()
    };

    // If a previous run already installed everything we need, jump straight to
    // the env step. With --reinstall we wipe everything and start over.
    if reinstall {
        let _ = std::fs::remove_dir_all(&version_root_folder);
        // Also discard any cached archive so the download starts fresh, not
        // from a possibly-corrupt leftover.
        if let Ok(p) = cache_path(&source_code_filename) {
            let _ = std::fs::remove_file(p);
        }
        if let Ok(p) = cache_path(format!("{source_code_filename}.part")) {
            let _ = std::fs::remove_file(p);
        }
        if let Ok(p) = cache_path(format!("{source_code_filename}.size")) {
            let _ = std::fs::remove_file(p);
        }
    } else if let Some(record) = read_record(version) {
        if record.matches(components, targets) {
            let mut tasks = Tasks::new();
            let t_env = tasks
                .new_task("Configuring shell")
                .wrap_err("Cannot report progress")?;
            let env_value = configure_shell(env_var, version, &t_env)?;
            t_env.finish();

            report.stage("source", StageStatus::Skipped("already installed".into()));
            report.stage("configure", StageStatus::Skipped("already installed".into()));
            report.stage("build", StageStatus::Skipped("already installed".into()));
            report.stage("install", StageStatus::Skipped("already installed".into()));
            report.stage("env", StageStatus::Done(format!("set {env_var}={env_value}")));
            report.env_value = env_value;
            return Ok(report);
        }
    }

    // Only needed when we actually have to build.
    let cmake = search_cmake()
        .wrap_err("'cmake' cannot be found")
        .with_suggestion(super::suggest_install_cmake)?;
    let generator = get_cmake_default_generator(cmake)?;

    let mut tasks = Tasks::new();

    let t0 = tasks
        .new_task(source_code_filename.as_str())
        .wrap_err("Cannot report progress")?;
    let t1 = tasks
        .new_task("Configuration")
        .wrap_err("Cannot report progress")?;
    let t2 = tasks
        .new_task("Compilation")
        .wrap_err("Cannot report progress")?;
    let t3 = tasks
        .new_task("Installation")
        .wrap_err("Cannot report progress")?;
    let t4 = tasks
        .new_task("Configuring shell")
        .wrap_err("Cannot report progress")?;

    // --- Download + extract source -------------------------------------------
    if source_present(version) {
        t0.finish_with_message("source already extracted");
        report.stage(
            "source",
            StageStatus::Skipped("source already extracted".into()),
        );
    } else {
        // Discard any partial extraction from a previous interrupted run so the
        // fresh extract does not mix with stale files.
        let _ = std::fs::remove_dir_all(&llvm_source_code_folder);
        let _ = dir_inside_cache_folder(format!("{version}/src"))?;
        let llvm_tar_gz_file_path =
            download_ungz_untar(&t0, source_code_url, llvm_source_code_folder)
                .await
                .wrap_err("Downloading source code")
                .with_suggestion(|| {
                    "a corrupt or incomplete archive was removed; re-run `llvmgr install` \
                     to download it again"
                })?;
        t0.set_subtask("Cleaning downloaded archive...");
        let _ = std::fs::remove_file(llvm_tar_gz_file_path);
        t0.finish();
        report.stage(
            "source",
            StageStatus::Done(format!("downloaded & extracted {source_code_filename}")),
        );
    }

    // --- Configure -----------------------------------------------------------
    let is_visual_studio = generator.contains("Visual Studio");
    let generator_label = if is_visual_studio {
        generator.clone()
    } else {
        "Ninja".to_string()
    };
    set_current_dir_inside_cache_folder(format!("{version}/src/build"))?;

    let cfg_matches = read_cfg_marker(version)
        .map(|m| m.matches(components, targets))
        .unwrap_or(false);

    if cfg_matches && build_configured(version) {
        t1.finish_with_message("already configured");
        report.stage(
            "configure",
            StageStatus::Skipped("already configured for these components".into()),
        );
    } else {
        let enable_projects = requested_components.join(";");

        if is_visual_studio {
            let mut args: Vec<String> = vec!["../llvm".into()];
            if !enable_projects.is_empty() {
                args.push(format!("-DLLVM_ENABLE_PROJECTS={enable_projects}"));
            }
            args.push(format!("-DLLVM_TARGETS_TO_BUILD={targets}"));
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            spawn_cmake(&t1, arg_refs)?;
        } else {
            let mut args: Vec<String> = vec![
                "../llvm".into(),
                "-DCMAKE_BUILD_TYPE=Release".into(),
                "-G".into(),
                "Ninja".into(),
            ];
            if !enable_projects.is_empty() {
                args.push(format!("-DLLVM_ENABLE_PROJECTS={enable_projects}"));
            }
            args.push(format!("-DLLVM_TARGETS_TO_BUILD={targets}"));
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            spawn_cmake(&t1, arg_refs)?;
        }
        write_cfg_marker(
            version,
            &InstallRecord {
                components: requested_components.clone(),
                targets: targets.to_string(),
            },
        )?;
        t1.finish();
        report.stage(
            "configure",
            StageStatus::Done(format!(
                "{generator_label}, projects=[{}], targets={targets}",
                if requested_components.is_empty() {
                    "core".to_string()
                } else {
                    requested_components.join(",")
                }
            )),
        );
    }

    // --- Build ---------------------------------------------------------------
    // Ninja resumes incrementally from wherever a previous build stopped, so we
    // always invoke it; a fully built tree is a fast no-op.
    if is_visual_studio {
        let cpus = if let Ok(cpus) = std::env::var("NUMBER_OF_PROCESSORS") {
            cpus.parse::<usize>().unwrap_or(1)
        } else {
            1
        };
        spawn_cmake(
            &t2,
            [
                "--build",
                ".",
                "--config",
                "Release",
                "-j",
                &cpus.to_string(),
            ],
        )?;
    } else {
        spawn_cmake(&t2, ["--build", "."])?;
    }
    t2.finish();
    report.stage(
        "build",
        StageStatus::Done(format!("compiled with {generator_label}")),
    );

    // --- Install -------------------------------------------------------------
    spawn_cmake(
        &t3,
        [
            &format!("-DCMAKE_INSTALL_PREFIX={}", version_root_folder.display()),
            "-P",
            "cmake_install.cmake",
        ],
    )?;
    write_record(
        version,
        &InstallRecord {
            components: requested_components,
            targets: targets.to_string(),
        },
    )?;
    t3.finish();
    report.stage(
        "install",
        StageStatus::Done(format!("installed to {}", version_root_folder.display())),
    );

    // --- Env vars ------------------------------------------------------------
    let env_value = configure_shell(env_var, version, &t4)?;
    t4.finish();
    report.stage("env", StageStatus::Done(format!("set {env_var}={env_value}")));
    report.env_value = env_value;

    Ok(report)
}

fn configure_shell(
    env_var: &str,
    version: &str,
    t: &crate::tasks::TaskRef,
) -> Result<String, Report> {
    t.set_subtask("configuring shell");
    let mut shell = read_shell().wrap_err("reading shell configuration")?;
    let prefix = dir_inside_cache_folder(version)?.display().to_string();
    let var = shell.env_vars.entry(env_var.into()).or_default();
    *var = prefix.clone();
    write_shell(&shell).wrap_err("writing shell configuration")?;
    Ok(prefix)
}

pub async fn llvm_16() -> Result<InstallReport, Report> {
    let cache_root_version = dir_inside_cache_folder("16.0.1")?;
    let _ = std::fs::remove_dir_all(&cache_root_version);

    let mut tasks = Tasks::new();

    let cmake = search_cmake()
        .wrap_err("'cmake' cannot be found")
        .with_suggestion(super::suggest_install_cmake)?;
    let generator = get_cmake_default_generator(cmake)?;

    let t0 = tasks
        .new_task("llvm-16.0.1.src.tar.xz")
        .wrap_err("Cannot report progress")?;
    let t1 = tasks
        .new_task("cmake-16.0.1.src.tar.xz")
        .wrap_err("Cannot report progress")?;
    let t2 = tasks
        .new_task("third-party-16.0.1.src.tar.xz")
        .wrap_err("Cannot report progress")?;
    let t3 = tasks
        .new_task("Compilation")
        .wrap_err("Cannot report progress")?;
    let t4 = tasks
        .new_task("Cleaning")
        .wrap_err("Cannot report progress")?;
    let t5 = tasks
        .new_task("Env Vars")
        .wrap_err("Cannot report progress")?;

    // Download and uncompress files
    let url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-16.0.1/llvm-16.0.1.src.tar.xz";
    download_unxz_untar(&t0, url, dir_inside_cache_folder("16.0.1/llvm")?)
        .await
        .wrap_err("Processing llvm-16.0.1.src.tar.xz")?;
    t0.finish();

    let url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-16.0.1/cmake-16.0.1.src.tar.xz";
    download_unxz_untar(&t1, url, dir_inside_cache_folder("16.0.1/cmake")?).await?;
    t1.finish();

    let url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-16.0.1/third-party-16.0.1.src.tar.xz";
    download_unxz_untar(&t2, url, dir_inside_cache_folder("16.0.1/third-party")?).await?;
    t2.finish();

    // Delete downloaded files
    t4.set_subtask("llvm-16.0.1.src.tar.xz");
    let _ = std::fs::remove_file(cache_path("llvm-16.0.1.src.tar.xz")?);

    t4.set_subtask("cmake-16.0.1.src.tar.xz");
    let _ = std::fs::remove_file(cache_path("cmake-16.0.1.src.tar.xz")?);

    t4.set_subtask("third-party-16.0.1.src.tar.xz");
    let _ = std::fs::remove_file(cache_path("third-party-16.0.1.src.tar.xz")?);

    // Compile
    set_current_dir_inside_cache_folder("16.0.1/llvm/build")?;
    if generator.contains("Visual Studio") {
        let cpus = if let Ok(cpus) = std::env::var("NUMBER_OF_PROCESSORS") {
            cpus.parse::<usize>().unwrap_or(1)
        } else {
            1
        };

        spawn_cmake(&t3, ["..", "-DLLVM_ENABLE_PROJECTS=lld;clang"])?;
        spawn_cmake(
            &t3,
            [
                "--build",
                ".",
                "--config",
                "Release",
                "-j",
                &cpus.to_string(),
            ],
        )?;

        // Move outputs
        t4.set_subtask("bin");
        super::move_dir(
            cache_path("16.0.1/llvm/build/Release/bin")?,
            cache_path("16.0.1")?,
        )?;

        t4.set_subtask("lib");
        super::move_dir(
            cache_path("16.0.1/llvm/build/Release/lib")?,
            cache_path("16.0.1")?,
        )?;

        t4.set_subtask("include");
        super::move_dir(cache_path("16.0.1/llvm/include")?, cache_path("16.0.1")?)?;
    } else {
        spawn_cmake(
            &t3,
            [
                "..",
                "-DCMAKE_BUILD_TYPE=Release",
                "-G",
                "Ninja",
                "-DLLVM_ENABLE_PROJECTS=lld;clang",
            ],
        )?;
        spawn_cmake(&t3, ["--build", "."])?;
        spawn_cmake(
            &t3,
            [
                &format!("-DCMAKE_INSTALL_PREFIX={}", cache_root_version.display()),
                "-P",
                "cmake_install.cmake",
            ],
        )?;

        // Move outputs
        t4.set_subtask("bin");
        super::move_dir(cache_path("16.0.1/llvm/build/bin")?, cache_path("16.0.1")?)?;

        t4.set_subtask("lib");
        super::move_dir(cache_path("16.0.1/llvm/build/lib")?, cache_path("16.0.1")?)?;

        t4.set_subtask("include");
        super::move_dir(cache_path("16.0.1/llvm/build/include")?, cache_path("16.0.1")?)?;
    }

    // Clean source code
    t4.set_subtask("llvm");
    super::remove_dir(cache_path("16.0.1/llvm")?)?;
    t4.set_subtask("cmake");
    super::remove_dir(cache_path("16.0.1/cmake")?)?;
    t4.set_subtask("third-party");
    super::remove_dir(cache_path("16.0.1/third-party")?)?;
    t4.finish();

    // Setup env vars
    t5.set_subtask("configuring shell");
    let mut shell = read_shell()?;
    let prefix = dir_inside_cache_folder("16.0.1")?.display().to_string();
    let var = shell
        .env_vars
        .entry("LLVM_SYS_160_PREFIX".into())
        .or_default();
    *var = prefix.clone();
    write_shell(&shell)?;
    t5.finish();

    let mut report = InstallReport {
        version: "16.0.1".to_string(),
        install_prefix: cache_root_version.display().to_string(),
        env_var: "LLVM_SYS_160_PREFIX".to_string(),
        env_value: prefix.clone(),
        components: vec!["clang".into(), "lld".into()],
        targets: "all".to_string(),
        ..Default::default()
    };
    report.stage("source", StageStatus::Done("downloaded & extracted split tarballs".into()));
    report.stage("configure", StageStatus::Done("cmake".into()));
    report.stage("build", StageStatus::Done("compiled".into()));
    report.stage("install", StageStatus::Done("moved bin/lib/include into prefix".into()));
    report.stage("env", StageStatus::Done(format!("set LLVM_SYS_160_PREFIX={prefix}")));

    Ok(report)
}