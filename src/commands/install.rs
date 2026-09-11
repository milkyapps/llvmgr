use super::llvm::{default_components, install_version, lookup_version, parse_components};
use crate::InstallSubcommand;
use color_eyre::eyre::{Report, WrapErr};

pub(crate) async fn run(_args: &crate::Args, install: &InstallSubcommand) -> Result<(), Report> {
    if install.name != "llvm" {
        return Err(color_eyre::eyre::eyre!(
            "unknown tool `{}`; only `llvm` is supported",
            install.name
        ));
    }

    // Legacy versions that predate the unified registry / install_version path.
    match install.version.as_str() {
        "16" => {
            let report = super::llvm::llvm_16()
                .await
                .wrap_err_with(|| "Unable to install llvm 16.0.1")?;
            report.print_summary();
            return Ok(());
        }
        "17" | "17.0.6" => {
            let components = match install.components.as_deref() {
                Some(list) => parse_components(list)?,
                None => default_components(),
            };
            let targets = install.targets.clone().unwrap_or_else(|| "Native".to_string());
            let report = install_version(
                "17.0.6",
                "LLVM_SYS_170_PREFIX",
                &components,
                &targets,
                install.reinstall,
            )
            .await
            .wrap_err_with(|| "Unable to install llvm 17.0.6")?;
            report.print_summary();
            return Ok(());
        }
        _ => {}
    }

    let spec = lookup_version(&install.version).ok_or_else(|| {
        let available: Vec<&str> = super::llvm::VERSIONS.iter().map(|v| v.major_minor).collect();
        color_eyre::eyre::eyre!(
            "unsupported version `{}`. Supported versions: 16, 17, {}",
            install.version,
            available.join(", ")
        )
    })?;

    let components = match install.components.as_deref() {
        Some(list) => parse_components(list)?,
        None => default_components(),
    };

    let targets = install.targets.clone().unwrap_or_else(|| "Native".to_string());

    let report = install_version(
        spec.full,
        spec.env_var,
        &components,
        &targets,
        install.reinstall,
    )
    .await
    .wrap_err_with(|| format!("Unable to install llvm {}", spec.full))?;
    report.print_summary();
    Ok(())
}