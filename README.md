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
info: Wrote /home/ryo/src/local/foo/template/Cargo.toml
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
$ cargo scripts new my-script
info: Running `/home/ryo/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo metadata --no-deps --format-version 1 --color auto --frozen`
info: Copied /home/ryo/src/local/foo/template/src/main.rs to /home/ryo/src/local/foo/my-script/src/main.rs
info: `package.name`: "template" → "my-script"
info: Wrote /home/ryo/src/local/foo/my-script/Cargo.toml
info: Added to "my-script" to `workspace.members`
info: Wrote /home/ryo/src/local/foo/Cargo.toml
$ tree
.
├── cargo-scripts.toml
├── Cargo.toml
├── my-script
│   ├── Cargo.toml
│   └── src
│       └── main.rs
└── template
    ├── Cargo.toml
    └── src
        └── main.rs

4 directories, 6 files
$ cat my-script/Cargo.toml
[package]
name = "my-script"
version = "0.0.0"
authors = ["Ryo Yamashita <qryxip@gmail.com>"]
edition = "2018"
publish = false

# See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

[dependencies]
$ cat my-script/src/main.rs
#!/usr/bin/env run-cargo-script
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
$ cargo scripts export my-script
info: Running `/home/ryo/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo metadata --no-deps --format-version 1 --color auto --frozen`
#!/usr/bin/env run-cargo-script
//! This is a regular crate doc comment, but it also contains a partial
//! Cargo manifest.  Note the use of a *fenced* code block, and the
//! `cargo` "language".
//!
//! ```cargo
//! [package]
//! name = "my-script"
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
$ curl -s https://api.github.com/gists/06bf1a7f4c65aec338003817de8e2074 | jq -rj '.files."hello-world.rs".content'
#!/usr/bin/env run-cargo-script
//! This code is licensed under [CC0-1.0](https://creativecommons.org/publicdomain/zero/1.0).
//!
//! ```cargo
//! [package]
//! name = "hello-world"
//! version = "0.0.0"
//! authors = ["Ryo Yamashita <qryxip@gmail.com>"]
//! edition = "2018"
//! publish = false
//! license = "CC0-1.0"
//!
//! [dependencies]
//! ```

fn main() {
    println!("Hello, World!");
}
$ cargo scripts gist import 06bf1a7f4c65aec338003817de8e2074
info: Running `/home/ryo/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo metadata --no-deps --format-version 1 --color auto --frozen`
info: GET: https://api.github.com/gists/06bf1a7f4c65aec338003817de8e2074
info: 200 OK
info: Wrote /home/ryo/src/local/foo/hello-world/Cargo.toml
info: Wrote /home/ryo/src/local/foo/hello-world/src/main.rs
info: Added to "hello-world" to `workspace.members`
info: Wrote /home/ryo/src/local/foo/Cargo.toml
info: `gist_ids."hello-world"`: Some("06bf1a7f4c65aec338003817de8e2074") -> "06bf1a7f4c65aec338003817de8e2074"
info: Wrote /home/ryo/src/local/foo/cargo-scripts.toml
$ tree
.
├── cargo-scripts.toml
├── Cargo.toml
├── hello-world
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── my-script
│   ├── Cargo.toml
│   └── src
│       └── main.rs
└── template
    ├── Cargo.toml
    └── src
        └── main.rs

6 directories, 8 files
$ cat ./hello-world/src/main.rs
#!/usr/bin/env run-cargo-script
//! This code is licensed under [CC0-1.0](https://creativecommons.org/publicdomain/zero/1.0).
//!
//! ```cargo
//! # Leave blank.
//! ```

fn main() {
    println!("Hello, World!");
}
$ cargo run -q --bin hello-world
Hello, World!
```

## License

Licensed under <code>[MIT](https://opensource.org/licenses/MIT) OR [Apache-2.0](http://www.apache.org/licenses/LICENSE-2.0)</code>.
