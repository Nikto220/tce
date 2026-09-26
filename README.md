<div align="center">
<img src="./assets/logo_tce.png" width="128px"/>

<br />
<br />

[![Current Crates.io Version](https://img.shields.io/crates/v/tce.svg)](https://crates.io/crates/tce)
![license](https://shields.io/badge/license-MIT%2FApache--2.0-blue)
<br />

---

**A Terrible Chess Engine.**
<br/>

</div>

# TL;DR

TCE is a Terrible Chess Engine that was created as a personal project to learn how chess engines work.

# Installation

You can install binaries from [here][https://github.com/Nikto220/tce/releases].
You can also run:
```
cargo install tce
```
But remember that cargo will compile the non-PEXT version without any flags, so if you want the PEXT version and you have a BMI2 CPU, you can set your rust compiler flags through `RUSTFLAGS="-C target-cpu=native"` (or `$env:RUSTFLAGS="-C target-cpu=native"` if you use powershell).

# Contributing

I probably won't accept PRs, but if you find any issues, feel free to report them!

# License

TCE is dual licensed under either MIT or Apache 2.0, at your option.
