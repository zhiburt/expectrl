[![Build](https://github.com/zhiburt/expectrl/actions/workflows/ci.yml/badge.svg)](https://github.com/zhiburt/expectrl/actions/workflows/ci.yml)
[![coverage status](https://coveralls.io/repos/github/zhiburt/expectrl/badge.svg?branch=main)](https://coveralls.io/github/zhiburt/expectrl?branch=main)
[![crate](https://img.shields.io/crates/v/expectrl)](https://crates.io/crates/expectrl)
[![docs.rs](https://img.shields.io/docsrs/expectrl?color=blue)](https://docs.rs/expectrl/*/expectrl/)

# expectrl

A tool for automating terminal applications on Unix and on Windows.

Using the library you can:

- Spawn process
- Control process
- Interact with process's IO(input/output).

`expectrl` like original `expect` may shine when you're working with interactive applications.
If your application is not interactive you may not find the library the best choise.

## Usage

A general example where the program simulates a used interacting with `ftp`.

```rust
use expectrl::{spawn, Regex, Eof, WaitStatus};

fn main() {
    let mut p = spawn("ftp speedtest.tele2.net").unwrap();
    p.expect(Regex("Name \\(.*\\):")).unwrap();
    p.send_line("anonymous").unwrap();
    p.expect("Password").unwrap();
    p.send_line("test").unwrap();
    p.expect("ftp>").unwrap();
    p.send_line("cd upload").unwrap();
    p.expect("successfully changed.\r\nftp>").unwrap();
    p.send_line("pwd").unwrap();
    p.expect(Regex("[0-9]+ \"/upload\"")).unwrap();
    p.send_line("exit").unwrap();
    p.expect(Eof).unwrap();
    assert_eq!(p.wait().unwrap(), WaitStatus::Exited(p.pid(), 0));
}
```

*The example inspired by the one in [philippkeller/rexpect].*

#### [For more examples, check the examples directory.](https://github.com/zhiburt/expectrl/tree/main/examples)

## Features

- It has an `async` support (To enable them you must turn on an `async` feature).
- It supports logging.
- It supports interact function.
- It has a Windows support.

## Notes

It was originally inspired by [philippkeller/rexpect] and [pexpect].

Licensed under [MIT License](LICENSE)

[philippkeller/rexpect]: https://github.com/philippkeller/rexpect
[pexpect]: https://pexpect.readthedocs.io/en/stable/overview.html
