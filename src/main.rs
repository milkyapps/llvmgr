mod commands;
mod tasks;

use argp::FromArgs;
use color_eyre::{eyre::Report, eyre::WrapErr};
use commands::read_shell;

/// Download, compile and install a specific LLVM version
#[derive(FromArgs, PartialEq, Debug)]
#[argp(subcommand, name = "install")]
struct InstallSubcommand {
    /// Options: llvm
    #[argp(positional)]
    name: String,

    /// LLVM version to install: a major (19), major.minor (19.1) or full
    /// version (19.1.7). Supported lines: 18.1, 19.1, 20.1, 21.1, 22.1, 23.1.
    /// The legacy `16` is also supported.
    #[argp(positional)]
    version: String,

    /// Components to build (comma-separated). Available: clang, lld, lldb,
    /// clang-tools-extra, compiler-rt, polly, flang, bolt. Default: clang,lld.
    /// Use this to build only what you need, e.g. `--components lld`.
    #[argp(option, short = 'c', long = "components")]
    components: Option<String>,

    /// LLVM targets to build (comma-separated), e.g. `Native` or
    /// `X86,AArch64`. Default: Native.
    #[argp(option, short = 't', long = "targets")]
    targets: Option<String>,

    /// Reinstall from scratch, discarding any previous progress.
    #[argp(switch, short = 'r', long = "reinstall")]
    reinstall: bool,
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
