use super::{
    cache_path, dir_inside_cache_folder, download_ungz_untar, download_unxz_untar,
    get_cmake_default_generator, move_dir, read_shell, remove_dir, search_cmake,
    set_current_dir_inside_cache_folder, spawn_cmake, write_shell,
};
use crate::tasks::Tasks;
use color_eyre::{
    eyre::WrapErr,
    eyre::{ContextCompat, Report},
    Help,
};

pub fn download_url(version: &str) -> (String, String) {
    (
        format!("https://github.com/llvm/llvm-project/archive/refs/tags/llvmorg-{version}.tar.gz"),
        format!("llvmorg-{version}.tar.gz"),
    )
}

pub async fn llvm_16() -> Result<(), Report> {
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
    *var = dir_inside_cache_folder("16.0.1")?.display().to_string();
    write_shell(&shell)?;
    t5.finish();

    Ok(())
}

pub async fn llvm_17() -> Result<(), Report> {
    let version = "17.0.6";

    let version_root_folder = dir_inside_cache_folder(version)?;
    let llvm_source_code_folder = dir_inside_cache_folder(format!("{version}/src"))?;

    let (source_code_url, source_code_filename) = download_url(version);

    let mut tasks = Tasks::new();

    let cmake = search_cmake()
        .wrap_err("'cmake' cannot be found")
        .with_suggestion(super::suggest_install_cmake)?;
    let generator = get_cmake_default_generator(cmake)?;

    let t0 = tasks
        .new_task(source_code_filename.as_str())
        .wrap_err("Cannot report progress")?;
    let t1 = tasks
        .new_task("Compilation")
        .wrap_err("Cannot report progress")?;
    let t2 = tasks
        .new_task("Installation")
        .wrap_err("Cannot report progress")?;
    let t3 = tasks
        .new_task("Configuring shell")
        .wrap_err("Cannot report progress")?;

    let _ = std::fs::remove_dir_all(&version_root_folder);

    // Download and uncompress source code
    // 194990759 bytes
    let llvm_tar_gz_file_path = download_ungz_untar(&t0, source_code_url, llvm_source_code_folder)
        .await
        .wrap_err("Downloading source code")?;
    t0.set_subtask("Cleaning downloaded files...");
    let _ = std::fs::remove_file(llvm_tar_gz_file_path);
    t0.finish();

    // Compilation
    set_current_dir_inside_cache_folder("17.0.6/src/build")?;
    if generator.contains("Visual Studio") {
        let cpus = if let Ok(cpus) = std::env::var("NUMBER_OF_PROCESSORS") {
            cpus.parse::<usize>().unwrap_or(1)
        } else {
            1
        };

        spawn_cmake(&t1, ["../llvm", "-DLLVM_ENABLE_PROJECTS=lld;clang"])?;
        spawn_cmake(
            &t1,
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
        spawn_cmake(
            &t1,
            [
                "../llvm",
                "-DCMAKE_BUILD_TYPE=Release",
                "-G",
                "Ninja",
                "-DLLVM_ENABLE_PROJECTS=lld;clang",
            ],
        )?;
        spawn_cmake(&t1, ["--build", "."])?;
    }

    // Installation
    spawn_cmake(
        &t2,
        [
            &format!("-DCMAKE_INSTALL_PREFIX={}", version_root_folder.display()),
            "-P",
            "cmake_install.cmake",
        ],
    )?;

    // Setup env vars
    t3.set_subtask("configuring shell");
    let mut shell = read_shell()?;
    let var = shell
        .env_vars
        .entry("LLVM_SYS_170_PREFIX".into())
        .or_default();
    *var = dir_inside_cache_folder("17.0.6")?.display().to_string();
    write_shell(&shell)?;
    t3.finish();

    Ok(())
}

pub async fn llvm_18() -> Result<(), Report> {
    let version = "18.1.2";

    let version_root_folder = dir_inside_cache_folder(version)?;
    let llvm_source_code_folder = dir_inside_cache_folder(format!("{version}/src"))?;

    let (source_code_url, source_code_filename) = download_url(version);

    let mut tasks = Tasks::new();

    let cmake = search_cmake()
        .wrap_err("'cmake' cannot be found")
        .with_suggestion(super::suggest_install_cmake)?;
    let generator = get_cmake_default_generator(cmake)?;

    let t0 = tasks
        .new_task(source_code_filename.as_str())
        .wrap_err("Cannot report progress")?;
    let t1 = tasks
        .new_task("Compilation")
        .wrap_err("Cannot report progress")?;
    let t2 = tasks
        .new_task("Installation")
        .wrap_err("Cannot report progress")?;
    let t3 = tasks
        .new_task("Configuring shell")
        .wrap_err("Cannot report progress")?;

    let _ = std::fs::remove_dir_all(&version_root_folder);

    // Download and uncompress source code
    // 205541214 bytes
    let llvm_tar_gz_file_path = download_ungz_untar(&t0, source_code_url, llvm_source_code_folder)
        .await
        .wrap_err("Downloading source code")?;
    t0.set_subtask("Cleaning downloaded files...");
    let _ = std::fs::remove_file(llvm_tar_gz_file_path);
    t0.finish();

    // Compilation
    set_current_dir_inside_cache_folder(format!("{version}/src/build"))?;
    if generator.contains("Visual Studio") {
        let cpus = if let Ok(cpus) = std::env::var("NUMBER_OF_PROCESSORS") {
            cpus.parse::<usize>().unwrap_or(1)
        } else {
            1
        };

        spawn_cmake(&t1, ["../llvm", "-DLLVM_ENABLE_PROJECTS=lld;clang"])?;
        spawn_cmake(
            &t1,
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
        spawn_cmake(
            &t1,
            [
                "../llvm",
                "-DCMAKE_BUILD_TYPE=Release",
                "-G",
                "Ninja",
                "-DLLVM_ENABLE_PROJECTS=lld;clang",
            ],
        )?;
        spawn_cmake(&t1, ["--build", "."])?;
    }

    // Installation
    spawn_cmake(
        &t2,
        [
            &format!("-DCMAKE_INSTALL_PREFIX={}", version_root_folder.display()),
            "-P",
            "cmake_install.cmake",
        ],
    )?;

    // Setup env vars
    t3.set_subtask("configuring shell");
    let mut shell = read_shell()?;
    let var = shell
        .env_vars
        .entry("LLVM_SYS_180_PREFIX".into())
        .or_default();
    *var = dir_inside_cache_folder(version)?.display().to_string();
    write_shell(&shell)?;
    t3.finish();

    Ok(())
}
