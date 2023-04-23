mod commands;
mod tasks;

use argp::FromArgs;
use color_eyre::{eyre::Report, eyre::WrapErr};
use commands::read_shell;

/// Instal LLVM tools
#[derive(FromArgs, PartialEq, Debug)]
#[argp(subcommand, name = "install")]
struct InstallSubcommand {
    #[argp(positional)]
    name: String,
    #[argp(positional)]
    version: String,
}

/// Setup shell environment variables
#[derive(FromArgs, PartialEq, Debug)]
#[argp(subcommand, name = "env")]
struct EnvSubcommand {
    #[argp(positional)]
    shell: String,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argp(subcommand)]
enum Commands {
    Install(InstallSubcommand),
    Env(EnvSubcommand),
}

/// LLVM Manager downloads, compiles and installs LLVM tools for you.
#[derive(FromArgs, PartialEq, Debug)]
struct Args {
    /// Be verbose.
    #[argp(switch, short = 'v', global)]
    verbose: bool,

    #[argp(subcommand)]
    command: Commands,
}
#[tokio::main]
async fn main() -> Result<(), Report> {
    color_eyre::install().unwrap();

    let args: Args = argp::parse_args_or_exit(argp::DEFAULT);

    match &args.command {
        Commands::Install(install) => commands::install::run(&args, install)
            .await
            .wrap_err_with(|| format!("Unable to install {} {}", install.name, install.version)),
        Commands::Env(_) => {
            let shell = read_shell().wrap_err("Unable to read shel configuration")?;
            for (k, v) in shell.env_vars {
                println!("export {k}={v}",);
            }
            Ok(())
        }
    }
}
