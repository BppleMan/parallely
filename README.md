# parallely

parallely is a command line process parallelization executor.

# preview

![preview](https://github.com/BppleMan/parallely/blob/main/readme/preview.png)

# install

```bash
cargo install parallely
```

# usage

### `parallely --help`

```plaintext
parallely is a command line process parallelization executor.

Usage: parallely [OPTIONS]

Options:
  -c, --commands <COMMANDS>  The commands to run in parallel. e.g. `parallely -c echo hello -c echo world`
      --eoc                  Exit on all sub-processes complete
  -d, --debug                Write log into $(PWD)/logs
  -h, --help                 Print help
  -V, --version              Print version
```

### `parallely -c echo hello -c echo world`

no-exit on all sub-processes complete

### `parallely -c echo hello -c echo world --eoc`

exit on all sub-processes complete

### `parallely -c echo hello -c echo world --debug`

write log into $(PWD)/logs

# limitation

* parallely will not process the standard input for a single command for you, but only forward the stdout/stderr of
  the child process to the output block.
* parallely can handle standard ansi-color output, but cannot support complete tty commands, such as clear and move
  cursor. Therefore, you cannot get the best experience for processes such as top and vim. Please try tmux/screen.
* parallely is more suitable for non-interactive pure output scenarios.
