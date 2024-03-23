mod commands;
mod tasks;

use argp::FromArgs;
use color_eyre::{eyre::Report, eyre::WrapErr};
use commands::read_shell;

/// Instal LLVM tools
#[derive(FromArgs, PartialEq, Debug)]
#[argp(subcommand, name = "install")]
struct InstallSubcommand {
    /// Options: llvm
    #[argp(positional)]
    name: String,

    /// Options: 16, 17, 18
    #[argp(positional)]
    version: String,
}

/// Setup shell environment variables
#[derive(FromArgs, PartialEq, Debug)]
#[argp(subcommand, name = "env")]
struct EnvSubcommand {
    /// Options: bash
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
        Commands::Install(cmd) => commands::install::run(&args, cmd)
            .await
            .wrap_err_with(|| format!("Unable to install {} {}", cmd.name, cmd.version)),
        Commands::Env(cmd) if cmd.shell == "bash" => {
            let shell = read_shell().wrap_err("Unable to read shell configuration")?;
            for (k, v) in shell.env_vars {
                println!("export {k}={v}",);
            }
            Ok(())
        }
        _ => todo!(),
    }
}
