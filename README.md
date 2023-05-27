A web interface for running Pony code.

Heavily based upon the [rust playpen](https://github.com/rust-lang/rust-playpen).

## Installation

See [INSTALL.md](INSTALL.md) for installation instructions for a real box.

## Running your own Pony-Playpen locally

### System Requirements

* Docker
* Rust (E.g. install via [Rustup](rustup.rs))

### Running the web server

First, create the Docker image that playpen will use:

```
docker build docker -t ponylang-playpen
```

Get a github personal access token. Only the `gist` scope needs to be selected.  
Put it into the `GITHUB_TOKEN` environment variable. 

```
 export GITHUB_TOKEN="..."
```

It will be used for creating gists with the playgrounds contents.
If you want to test without a valid token, really all you need to do 
is to set the variable to some gibberish.

Next, spin up the server.

```
cargo run --bin playpen
```

You should now be able to browse http://127.0.0.1:8000 and play.

