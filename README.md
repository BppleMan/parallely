# parallely

parallely is a command line process parallelization executor.

# preview

![preview](https://github.com/BppleMan/parallely/blob/main/readme/preview.png?raw=true)

# install

```bash
cargo install parallely
```

# usage

### `parallely --help`

```plaintext
parallely is a command line process parallelization executor.

Usage: parallely [OPTIONS] <COMMANDS>...

Arguments:
  <COMMANDS>...  The commands to run in parallel. e.g. `parallely "echo hello" "echo world"`

Options:
      --eoc      Exit on all sub-processes complete
  -d, --debug    Write log into $(PWD)/logs
  -h, --help     Print help
  -V, --version  Print version
```

### `parallely "echo hello" "echo world"`

no-exit on all sub-processes complete

### `parallely "echo hello" "echo world" --eoc`

exit on all sub-processes complete

### `parallely "echo hello" "echo world" --debug`

write log into $(PWD)/logs

# limitation

* parallely will not process the standard input for a single command for you, but only forward the stdout/stderr of
  the child process to the output block.
* parallely can handle standard ansi-color output, but cannot support complete tty commands, such as clear and move
  cursor. Therefore, you cannot get the best experience for processes such as top and vim. Please try tmux/screen.
* parallely is more suitable for non-interactive pure output scenarios.

# what's new

## v0.2.0

* \<feature\> auto scroll to the bottom when new output comes.
