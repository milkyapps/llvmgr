# llvmgr

Helps you to download, compile and install LLVM.  
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
  name           Options: llvm
  version        Options: 16, 17

Options:
  -v, --verbose  Be verbose.
  -h, --help     Show this help message and exit.
```

## Shell Integration at Linux

Suggestion is to source the output of `llvmgr env bash` at your `.bashrc`.

```
eval "$(llvmgr env bash)"
```

This will export all installed versions as `LLVM_SYS_*_PREFIX` environment variables.

```
> llvmgr env bash
export LLVM_SYS_160_PREFIX=/home/xunilrj/.cache/llvmgr/16.0.1
export LLVM_SYS_170_PREFIX=/home/xunilrj/.cache/llvmgr/17.0.6
```
