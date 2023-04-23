# llvmgr

Helps you to download, to compile and to install LLVM.  
Specially tailored for LLVM development with https://gitlab.com/taricorp/llvm-sys.rs

## Install

```
cargo install --git https://github.com/milkyapps/llvmgr
```

## Usage

```
> llvmgr install --help
Usage: llvmgr install [-v] <name> <version>

Instal LLVM tools

Arguments:
  name
  version

Options:
  -v, --verbose  Be verbose.
  -h, --help     Show this help message and exit.
```

## Shell Integration

Suggestion is to source the output of `llvmgr env bash` at your `.bashrc`.

```
eval "$(llvmgr env bash)"
```

This will export all installed versions as `LLVM_SYS_*_PREFIX` environment variables.

```
> llvmgr env bash
export LLVM_SYS_160_PREFIX=/home/user/.cache/llvmgr/16.0.1
```
