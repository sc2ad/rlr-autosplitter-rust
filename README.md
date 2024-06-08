# rlr-autosplitter-rust

An auto splitter for the [RLR Categories](https://www.speedrun.com/Runling_Run).

## Features

### RLR4

PLANNED:

- [x] Auto split on level completion
- [x] Auto split on boss completion
- [x] Auto split on general EXP increments
- [ ] Automatic timer deduction
- [ ] Auto-start logic

### RLR8

- TODO

## Settings

TODO

## Compilation

This auto splitter is written in Rust. In order to compile it, you need to
install the Rust compiler: [Install Rust](https://www.rust-lang.org/tools/install).

Afterwards install the WebAssembly target:

```sh
rustup target add wasm32-unknown-unknown --toolchain nightly
```

The auto splitter can now be compiled:

```sh
cargo b --release
```

The auto splitter is then available at:

```sh
target/wasm32-unknown-unknown/release/rlr_autosplitter_rust.wasm
```

Make sure to look into the [API documentation](https://livesplit.org/asr/asr/) for the `asr` crate.

## Development

You can use the [debugger](https://github.com/LiveSplit/asr-debugger) while
developing the auto splitter to more easily see the log messages, statistics,
dump memory, step through the code and more.

The repository comes with preconfigured Visual Studio Code tasks. During
development it is recommended to use the `Debug Auto Splitter` launch action to
run the `asr-debugger`. You need to install the `CodeLLDB` extension to run it.

You can then use the `Build Auto Splitter (Debug)` task to manually build the
auto splitter. This will automatically hot reload the auto splitter in the
`asr-debugger`.

Alternatively you can install the [`cargo
watch`](https://github.com/watchexec/cargo-watch?tab=readme-ov-file#install)
subcommand and run the `Watch Auto Splitter` task for it to automatically build
when you save your changes.

The debugger is able to step through the code. You can set breakpoints in VSCode
and it should stop there when the breakpoint is hit. Inspecting variables may
not work all the time.
