[![Build](https://github.com/zhiburt/expectrl/actions/workflows/ci.yml/badge.svg)](https://github.com/zhiburt/expectrl/actions/workflows/ci.yml)
[![coverage status](https://coveralls.io/repos/github/zhiburt/expectrl/badge.svg?branch=main)](https://coveralls.io/github/zhiburt/expectrl?branch=main)
[![crate](https://img.shields.io/crates/v/expectrl)](https://crates.io/crates/expectrl)
[![docs.rs](https://img.shields.io/docsrs/expectrl?color=blue)](https://docs.rs/expectrl/*/expectrl/)

# expectrl

Expectrl is a tool for automating terminal applications.

Expectrl is a rust module for spawning child applications and controlling them and responding to expected patterns in process's output. Expectrl works like Don Libes' Expect. Expectrl allows your script to spawn a child application and control it as if a human were typing commands.

Using the library you can:

- Spawn process
- Control process
- Interact with process's IO(input/output).

`expectrl` like original `expect` may shine when you're working with interactive applications.
If your application is not interactive you may not find the library the best choise.

## Usage

Add `expectrl` to your Cargo.toml.

```toml
# Cargo.toml
[dependencies]
pg-embed = { version = "0.7", default-features = false, features = ["rt_tokio"] }
```

An example where the program simulates a used interacting with `ftp`.

```rust
use expectrl::{spawn, Regex, Eof, WaitStatus, Error};

fn main() -> Result<(), Error> {
    let mut p = spawn("ftp speedtest.tele2.net")?;
    p.expect(Regex("Name \\(.*\\):"))?;
    p.send_line("anonymous")?;
    p.expect("Password")?;
    p.send_line("test")?;
    p.expect("ftp>")?;
    p.send_line("cd upload")?;
    p.expect("successfully changed.\r\nftp>")?;
    p.send_line("pwd")?;
    p.expect(Regex("[0-9]+ \"/upload\""))?;
    p.send_line("exit")?;
    p.expect(Eof)?;
    assert_eq!(p.wait()?, WaitStatus::Exited(p.pid(), 0));
}
```

*The example inspired by the one in [philippkeller/rexpect].*

The same exxample but the password will be read from stdin.

```rust
use expectrl::{spawn, Regex, Eof, WaitStatus, Error};

fn main() -> Result<(), Error> {
    let mut p = spawn("ftp speedtest.tele2.net")?;
    p.expect(Regex("Name \\(.*\\):"))?;
    p.send_line("anonymous")?;
    p.expect("Password")?;
    p.send_line("test")?;
    p.expect("ftp>")?;
    p.send_line("cd upload")?;
    p.expect("successfully changed.\r\nftp>")?;
    p.send_line("pwd")?;
    p.expect(Regex("[0-9]+ \"/upload\""))?;
    p.send_line("exit")?;
    p.expect(Eof)?;
    assert_eq!(p.wait()?, WaitStatus::Exited(p.pid(), 0));
}
```

#### [For more examples, check the examples directory.](https://github.com/zhiburt/expectrl/tree/main/examples)

## Features

- It has an `async` support (To enable them you must turn on an `async` feature).
- It supports logging.
- It supports interact function.
- It works on windows.

## Notes

It was originally inspired by [philippkeller/rexpect] and [pexpect].

Licensed under [MIT License](LICENSE)

[philippkeller/rexpect]: https://github.com/philippkeller/rexpect
[pexpect]: https://pexpect.readthedocs.io/en/stable/overview.html
