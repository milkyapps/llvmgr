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
Usage: llvmgr install [-v] [-c <components>] [-t <targets>] [-r] <name> <version>

Download, compile and install a specific LLVM version

Arguments:
  name                     Options: llvm
  version                  LLVM version to install: a major (19), major.minor
                           (19.1) or full version (19.1.7). Supported lines:
                           18.1, 19.1, 20.1, 21.1, 22.1, 23.1. The legacy `16`
                           is also supported.

Options:
  -v, --verbose            Be verbose.
  -c, --components <components>
                           Components to build (comma-separated). Available:
                           clang, lld, lldb, clang-tools-extra, compiler-rt,
                           polly, flang, bolt. Default: clang,lld. Use this to
                           build only what you need, e.g. `--components lld`.
  -t, --targets <targets>  LLVM targets to build (comma-separated), e.g.
                           `Native` or `X86,AArch64`. Default: Native.
  -r, --reinstall          Reinstall from scratch, discarding any previous
                           progress.
  -h, --help               Show this help message and exit.
```

### Supported versions

Every LLVM line from 18.1 onwards is supported, pinned to the latest known
patch release:

| Line | Release  | llvm-sys env var        |
|------|----------|-------------------------|
| 18.1 | 18.1.8   | `LLVM_SYS_180_PREFIX`   |
| 19.1 | 19.1.7   | `LLVM_SYS_190_PREFIX`   |
| 20.1 | 20.1.8   | `LLVM_SYS_200_PREFIX`   |
| 21.1 | 21.1.7   | `LLVM_SYS_210_PREFIX`   |
| 22.1 | 22.1.8   | `LLVM_SYS_220_PREFIX`   |
| 23.1 | 23.1.0   | `LLVM_SYS_230_PREFIX`   |

The legacy `16` (split source tarballs) is still supported.

You can pass a major version (`22`), a major.minor line (`22.1`) or a full
version (`22.1.8`); all three resolve to the same release.

### Choosing components

By default llvmgr builds `clang` and `lld` on top of LLVM core. If you only
need a subset, pass `--components` to skip building everything else — this is
the main lever for cutting build time and disk usage:

```
# only the linker
llvmgr install llvm 22 --components lld

# clang, lld and the extra tooling
llvmgr install llvm 21 --components clang,lld,clang-tools-extra
```

`--targets` controls `LLVM_TARGETS_TO_BUILD` and defaults to `Native` (only the
host target), which is much faster than building every target. Pass a
comma-separated list to cross-target:

```
llvmgr install llvm 22 --targets X86,AArch64
```

### Resumable installs

Installs are resumable: each stage (download/extract, configure, compile,
install) is skipped when its output is already on disk, and the build itself is
incremental (Ninja resumes from where it stopped). A `.llvmgr_installed` marker
records which components/targets have already been installed, so re-running the
same command is a no-op and adding a new component only builds what is missing.

If you ever need a clean rebuild:

```
llvmgr install llvm 22 --reinstall
```

## Shell Integration at Linux

Suggestion is to source the output of `llvmgr env bash` at your `.bashrc`.

```
eval "$(llvmgr env bash)"
```

This will export all installed versions as `LLVM_SYS_*_PREFIX` environment variables.

```
> llvmgr env bash
export LLVM_SYS_170_PREFIX=/home/xunilrj/.cache/llvmgr/17.0.6
export LLVM_SYS_180_PREFIX=/home/xunilrj/.cache/llvmgr/18.1.8
export LLVM_SYS_220_PREFIX=/home/xunilrj/.cache/llvmgr/22.1.8
```