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
      --wait                 
  -h, --help                 Print help
  -V, --version              Print version
```

### `parallely -c echo hello -c echo world`

exit when all commands are finished

### `parallely -c echo hello -c echo world --wait`

exit only press `q`/`ctrl-c` key
