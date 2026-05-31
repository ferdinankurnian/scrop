# scrop

`scrop` is a precise Wayland region selector for screen recorders and screenshot scripts.
Unlike one-drag selectors, it keeps the selected rectangle open so it can be moved, resized,
and confirmed explicitly.

It prints a [`slurp`](https://github.com/emersion/slurp)-compatible geometry string to stdout,
so it can be used as a drop-in selector for tools such as
[`wl-screenrec`](https://github.com/russelltg/wl-screenrec).

## Install

Install build dependencies on Arch:

```sh
sudo pacman -S cairo pango wayland
```

Install `scrop` from a local checkout:

```sh
cargo install --path .
```

## Usage

Start the selector:

```sh
scrop
```

`scrop select` starts the same interactive selector explicitly:

```sh
scrop select
```

After confirmation, `scrop` prints geometry to stdout:

```text
120,80 1280x720
```

Use it with `wl-screenrec`:

```sh
wl-screenrec -g "$(scrop)"
```

## Controls

| Input | Action |
| --- | --- |
| Drag outside the selection | Create a new selection |
| Drag inside the selection | Move the selection |
| Drag a resize handle | Resize the selection |
| Click **Select** or press `Enter` | Confirm and print geometry |
| Click **x** or press `Escape` | Cancel |
| Arrow keys | Move the selection by 1 pixel |
| `Shift` + arrow keys | Move the selection by 10 pixels |

## CLI

```text
Usage: scrop [OPTIONS] [COMMAND]

Commands:
  select  Interactive region selection
  help    Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...  Increase diagnostic output; may be repeated
  -h, --help        Print help
  -V, --version     Print version
```

`-v` is repeatable. Diagnostics go to stderr so stdout remains suitable for command
substitution.

## Exit Codes

| Code | Meaning |
| --- | --- |
| `0` | A region was selected and printed |
| `1` | Selection was cancelled |
| `2` | Initialization or runtime error |

## Requirements

`scrop` is Wayland-only. The compositor must support:

- `wlr-layer-shell-unstable-v1`
- `zxdg-output-unstable-v1`

`cursor-shape-v1` is recommended for context-aware crosshair, move, and resize cursors.

Selections are constrained to one output because recorders such as `wl-screenrec` expect a region
fully contained within a single output.

## Development

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```

## Releases

GitHub Actions builds the x86_64 Arch Linux binary and publishes a tarball plus checksum when a
version tag is pushed. The tag must match the version in `Cargo.toml`.

```sh
git tag v0.1.0
git push origin v0.1.0
```

The release tarball is intended for the `scrop-bin` AUR package.
