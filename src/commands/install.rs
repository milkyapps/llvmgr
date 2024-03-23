use super::llvm::{llvm_16, llvm_17, llvm_18};
use crate::{Args, InstallSubcommand};
use color_eyre::eyre::Report;

#[derive(Debug)]
pub(crate) enum InstallError {}

pub(crate) async fn run(_: &Args, install: &InstallSubcommand) -> Result<(), Report> {
    match (install.name.as_str(), install.version.as_str()) {
        ("llvm", "16") => llvm_16().await,
        ("llvm", "17") => llvm_17().await,
        ("llvm", "18") => llvm_18().await,
        _ => todo!(),
    }
}
