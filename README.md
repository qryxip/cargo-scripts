# cargo-script**s**

[![CI](https://github.com/qryxip/cargo-scripts/workflows/CI/badge.svg)](https://github.com/qryxip/cargo-scripts/actions?workflow=CI)
[![codecov](https://codecov.io/gh/qryxip/cargo-scripts/branch/master/graph/badge.svg)](https://codecov.io/gh/qryxip/cargo-scripts/branch/master)
[![dependency status](https://deps.rs/repo/github/qryxip/cargo-scripts/status.svg)](https://deps.rs/repo/github/qryxip/cargo-scripts)
[![Crates.io](https://img.shields.io/badge/crates.io-not%20yet-inactive)](https://crates.io)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-informational)](https://crates.io)

A Cargo subcommand for managing Rust "script"s.

## Installation

`cargo-scripts` is not yet uploaded to [crates.io](https://crates.io).

```console
$ cargo install --git https://github.com/qryxip/cargo-scripts
```

## Usage

```console
$ cargo scripts --help
cargo-scripts 0.0.0
Ryo Yamashita <qryxip@gmail.com>
A Cargo subcommand for managing Rust "script"s.

USAGE:
    cargo scripts <SUBCOMMAND>

OPTIONS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    init-workspace    Create a new workspace in an existing directory
    new               Create a new workspace member from a template
    rm                Remove a workspace member
    include           Include a package in the workspace
    exclude           Exclude a package from the workspace
    import            Import a script as a package (in the same format as `cargo-script`)
    export            Export a package as a script (in the same format as `cargo-script`)
    gist              Gist
    config            Modify cargo-scripts.toml
    help              Prints this message or the help of the given subcommand(s)
$ cargo scripts gist --help
cargo-scripts-gist 0.0.0
Ryo Yamashita <qryxip@gmail.com>
Gist

USAGE:
    cargo scripts gist <SUBCOMMAND>

OPTIONS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    clone    Clone a script from Gist
    pull     Pull a script from Gist
    push     Pull a script to Gist
    help     Prints this message or the help of the given subcommand(s)
```

## Example

```console
$ pwd
/home/ryo/src/local/foo
$ tree
.

0 directories, 0 files
$ cargo scripts init-workspace
info: Wrote /home/ryo/src/local/foo/Cargo.toml
info: Wrote /home/ryo/src/local/foo/cargo-scripts.toml
info: Running `/home/ryo/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo new --vcs none /home/ryo/src/local/foo/template`
     Created binary (application) `/home/ryo/src/local/foo/template` package
info: `package.version`: "0.1.0" → "0.0.0"
info: `package.publish`: None → false
info: [dry-run] Wrote /home/ryo/src/local/foo/template/Cargo.toml
info: Wrote /home/ryo/src/local/foo/template/src/main.rs
$ tree
.
├── cargo-scripts.toml
├── Cargo.toml
└── template
    ├── Cargo.toml
    └── src
        └── main.rs

2 directories, 4 files
$ cargo scripts new hello-world
info: Running `/home/ryo/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo metadata --no-deps --format-version 1 --color auto --frozen`
info: Copied /home/ryo/src/local/foo/template/src/main.rs to /home/ryo/src/local/foo/hello-world/src/main.rs
info: `package.name`: "template" → "hello-world"
info: Wrote /home/ryo/src/local/foo/hello-world/Cargo.toml
info: Added to "hello-world" to `workspace.members`
info: Wrote /home/ryo/src/local/foo/Cargo.toml
$ tree
.
├── cargo-scripts.toml
├── Cargo.toml
├── hello-world
│   ├── Cargo.toml
│   └── src
│       └── main.rs
└── template
    ├── Cargo.toml
    └── src
        └── main.rs

4 directories, 6 files
$ cat ./hello-world/Cargo.toml
[package]
name = "hello-world"
version = "0.0.0"
authors = ["Ryo Yamashita <qryxip@gmail.com>"]
edition = "2018"
publish = false

​# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[dependencies]
$ cat ./hello-world/src/main.rs
​#!/usr/bin/env run-cargo-script
//! This is a regular crate doc comment, but it also contains a partial
//! Cargo manifest.  Note the use of a *fenced* code block, and the
//! `cargo` "language".
//!
//! ```cargo
//! # Leave blank.
//! ```

fn main() {
    todo!();
}
$ cargo scripts ​export hello-world
info: Running `/home/ryo/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo metadata --no-deps --format-version 1 --color auto --frozen`
​#!/usr/bin/env run-cargo-script
//! This is a regular crate doc comment, but it also contains a partial
//! Cargo manifest.  Note the use of a *fenced* code block, and the
//! `cargo` "language".
//!
//! ```cargo
//! [package]
//! name = "hello-world"
//! version = "0.0.0"
//! authors = ["Ryo Yamashita <qryxip@gmail.com>"]
//! edition = "2018"
//! publish = false
//!
//! # See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html
//!
//! [dependencies]
//! ```

fn main() {
    todo!();
}
$ cargo scripts gist push --set-upstream --description 'Generated and posted with https://github.com/qryxip/cargo-scripts' hello-world
info: Running `/home/ryo/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo metadata --no-deps --format-version 1 --color auto --frozen`
info: POST https://api.github.com/gists
info: Created `826239363224b3df28fc925618037c3a`
info: `gist_ids."hello-world"`: None →> "826239363224b3df28fc925618037c3a"
info: Wrote /home/ryo/src/local/foo/cargo-scripts.toml
$ curl -s https://api.github.com/gists/826239363224b3df28fc925618037c3a | jq -rj '.files."hello-world.rs".content'
​#!/usr/bin/env run-cargo-script
//! This is a regular crate doc comment, but it also contains a partial
//! Cargo manifest.  Note the use of a *fenced* code block, and the
//! `cargo` "language".
//!
//! ```cargo
//! [package]
//! name = "hello-world"
//! version = "0.0.0"
//! authors = ["Ryo Yamashita <qryxip@gmail.com>"]
//! edition = "2018"
//! publish = false
//!
//! # See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html
//!
//! [dependencies]
//! ```

fn main() {
    todo!();
}
$ sd -s 'todo!()' 'println!("Hello!")' ./hello-world/src/main.rs
$ cargo scripts gist push hello-world
info: Running `/home/ryo/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo metadata --no-deps --format-version 1 --color auto --frozen`
info: GET: https://api.github.com/gists/826239363224b3df28fc925618037c3a
info: 200 OK
info: PATCH https://api.github.com/gists/826239363224b3df28fc925618037c3a
info: 200 OK
info: Updated `826239363224b3df28fc925618037c3a`
$ # https://gist.github.com/qryxip/826239363224b3df28fc925618037c3a/revisions
```

## License

Licensed under <code>[MIT](https://opensource.org/licenses/MIT) OR [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0)</code>.
