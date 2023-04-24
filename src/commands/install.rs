use super::{
    cache_dir, cache_path, cache_set_current_dir, download_unxz_untar, get_cmake_default_generator,
    move_dir, read_shell, remove_dir, search_cmake, spawn_cmake, write_shell,
};
use crate::{tasks::Tasks, Args, InstallSubcommand};
use color_eyre::{
    eyre::WrapErr,
    eyre::{ContextCompat, Report},
    Help,
};

#[derive(Debug)]
pub(crate) enum InstallError {}

pub(crate) async fn run(_: &Args, install: &InstallSubcommand) -> Result<(), Report> {
    match (install.name.as_str(), install.version.as_str()) {
        ("llvm", "16") => llvm_16().await,
        _ => todo!(),
    }
}

async fn llvm_16() -> Result<(), Report> {
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
    download_unxz_untar(&t0, url, cache_dir("16.0.1/llvm")?)
        .await
        .wrap_err("Processing llvm-16.0.1.src.tar.xz")?;
    t0.finish();

    let url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-16.0.1/cmake-16.0.1.src.tar.xz";
    download_unxz_untar(&t1, url, cache_dir("16.0.1/cmake")?).await?;
    t1.finish();

    let url = "https://github.com/llvm/llvm-project/releases/download/llvmorg-16.0.1/third-party-16.0.1.src.tar.xz";
    download_unxz_untar(&t2, url, cache_dir("16.0.1/third-party")?).await?;
    t2.finish();

    // Delete downloaded files
    t4.set_subtask("llvm-16.0.1.src.tar.xz");
    let _ = std::fs::remove_file(cache_path("llvm-16.0.1.src.tar.xz")?);

    t4.set_subtask("cmake-16.0.1.src.tar.xz");
    let _ = std::fs::remove_file(cache_path("cmake-16.0.1.src.tar.xz")?);

    t4.set_subtask("third-party-16.0.1.src.tar.xz");
    let _ = std::fs::remove_file(cache_path("third-party-16.0.1.src.tar.xz")?);

    // Compile
    cache_set_current_dir("16.0.1/llvm/build")?;
    if generator.contains("Visual Studio") {
        let cpus = if let Ok(cpus) = std::env::var("NUMBER_OF_PROCESSORS") {
            cpus.parse::<usize>().unwrap_or(1)
        } else {
            1
        };

        spawn_cmake(&t3, [".."])?;
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
        move_dir(
            cache_path("16.0.1/llvm/build/Release/bin")?,
            cache_path("16.0.1")?,
        )?;

        t4.set_subtask("lib");
        move_dir(
            cache_path("16.0.1/llvm/build/Release/lib")?,
            cache_path("16.0.1")?,
        )?;

        t4.set_subtask("include");
        move_dir(cache_path("16.0.1/llvm/include")?, cache_path("16.0.1")?)?;
    } else {
        spawn_cmake(&t3, ["..", "-DCMAKE_BUILD_TYPE=Release", "-G", "Ninja"])?;
        spawn_cmake(&t3, ["--build", "."])?;

        // Move outputs
        t4.set_subtask("bin");
        move_dir(cache_path("16.0.1/llvm/build/bin")?, cache_path("16.0.1")?)?;

        t4.set_subtask("lib");
        move_dir(cache_path("16.0.1/llvm/build/lib")?, cache_path("16.0.1")?)?;

        t4.set_subtask("include");
        move_dir(
            cache_path("16.0.1/llvm/build/include")?,
            cache_path("16.0.1")?,
        )?;
    }

    // Clean source code
    t4.set_subtask("llvm");
    remove_dir(cache_path("16.0.1/llvm")?)?;
    t4.set_subtask("cmake");
    remove_dir(cache_path("16.0.1/cmake")?)?;
    t4.set_subtask("third-party");
    remove_dir(cache_path("16.0.1/third-party")?)?;
    t4.finish();

    // Setup env vars
    t5.set_subtask("configuring shell");
    let mut shell = read_shell()?;
    let var = shell
        .env_vars
        .entry("LLVM_SYS_160_PREFIX".into())
        .or_default();
    *var = cache_dir("16.0.1")?.display().to_string();
    write_shell(&shell)?;
    t5.finish();

    Ok(())
}
