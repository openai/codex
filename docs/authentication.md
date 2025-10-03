# Authentication

## Usage-based billing alternative: Use an OpenAI API key

If you prefer to pay-as-you-go, you can still authenticate with your OpenAI API key:

```shell
printenv OPENAI_API_KEY | codex login --with-api-key
```

Alternatively, read from a file:

```shell
codex login --with-api-key < my_key.txt
```

The legacy `--api-key` flag now exits with an error instructing you to use `--with-api-key` so that the key never appears in shell history or process listings.

This key must, at minimum, have write access to the Responses API.

## Migrating to ChatGPT login from API key

If you've used the Codex CLI before with usage-based billing via an API key and want to switch to using your ChatGPT plan, follow these steps:

1. Update the CLI and ensure `codex --version` is `0.20.0` or later
2. Delete `~/.codex/auth.json` (on Windows: `C:\\Users\\USERNAME\\.codex\\auth.json`)
3. Run `codex login` again

## Connecting on a "Headless" Machine

Today, the login process entails running a server on `localhost:1455`. If you are on a "headless" server, such as a Docker container or are `ssh`'d into a remote machine, loading `localhost:1455` in the browser on your local machine will not automatically connect to the webserver running on the _headless_ machine, so you must use one of the following workarounds:

### Authenticate locally and copy your credentials to the "headless" machine

The easiest solution is likely to run through the `codex login` process on your local machine such that `localhost:1455` _is_ accessible in your web browser. When you complete the authentication process, an `auth.json` file should be available at `$CODEX_HOME/auth.json` (on Mac/Linux, `$CODEX_HOME` defaults to `~/.codex` whereas on Windows, it defaults to `%USERPROFILE%\\.codex`).

Because the `auth.json` file is not tied to a specific host, once you complete the authentication flow locally, you can copy the `$CODEX_HOME/auth.json` file to the headless machine and then `codex` should "just work" on that machine. Note to copy a file to a Docker container, you can do:

```shell
# substitute MY_CONTAINER with the name or id of your Docker container:
CONTAINER_HOME=$(docker exec MY_CONTAINER printenv HOME)
docker exec MY_CONTAINER mkdir -p "$CONTAINER_HOME/.codex"
docker cp auth.json MY_CONTAINER:"$CONTAINER_HOME/.codex/auth.json"
```

whereas if you are `ssh`'d into a remote machine, you likely want to use [`scp`](https://en.wikipedia.org/wiki/Secure_copy_protocol):

```shell
ssh user@remote 'mkdir -p ~/.codex'
scp ~/.codex/auth.json user@remote:~/.codex/auth.json
```

or try this one-liner:

```shell
ssh user@remote 'mkdir -p ~/.codex && cat > ~/.codex/auth.json' < ~/.codex/auth.json
```

### Connecting through VPS or remote

If you run Codex on a remote machine (VPS/server) without a local browser, the login helper starts a server on `localhost:1455` on the remote host. To complete login in your local browser, forward that port to your machine before starting the login flow:

```bash
# From your local machine
ssh -L 1455:localhost:1455 <user>@<remote-host>
```

Then, in that SSH session, run `codex` and select "Sign in with ChatGPT". When prompted, open the printed URL (it will be `http://localhost:1455/...`) in your local browser. The traffic will be tunneled to the remote server. 
