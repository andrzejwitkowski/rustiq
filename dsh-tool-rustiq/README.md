# @andrzejwitkowski/dsh-tool-rustiq

Cordis plugin for [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness): `rustiq_*` tools drive the [rustiq](https://github.com/andrzejwitkowski/rustiq) TUI over `ctx.terminals` (terminal card in chat, not a sidebar). After `V`, comments are in `.rustiq/export.txt`.

## Install

1. Put `rustiq` on PATH (`cargo build --release` then copy the binary).
2. Ensure the Host also loads the PTY seam (`@deepseek-ai/dsh-terminal` + `@deepseek-ai/dsh-terminal-bash`).
3. Insert this plugin in the profile patch (`~/.dsh/profiles/web/cordis.patch.yml`):

```yaml
- insert:
    - id: dsh-terminal
      name: '@deepseek-ai/dsh-terminal'
    - id: dsh-terminal-bash
      name: '@deepseek-ai/dsh-terminal-bash'
    - id: tool-rustiq
      name: '/ABSOLUTE/PATH/TO/rustiq/dsh-tool-rustiq'
      config:
        binaryPath: rustiq
```

If `pty` / `dsh-terminal` rows already exist, insert only `tool-rustiq`. Restart `dsh web`. Relative `name` is resolved from the patch file directory.

## Tools

| Tool | Role |
|------|------|
| `rustiq_open` | Spawn a shell PTY and run `rustiq --inline` |
| `rustiq_send` | Keys (`submit: false`) or Enter-terminated text (`submit: true`) |
| `rustiq_read` | Current screen |
| `rustiq_close` | Kill the session |

`--inline` keeps rustiq off the alternate screen so Harness PTY reads see the UI (the stock PTY backend does not track alt-buffer).

## Config

| key | default | meaning |
|-----|---------|---------|
| `binaryPath` | `rustiq` | Name or absolute path |
| `maxResultBytes` | 262144 | Cap for rendered viewport text |

## Limitations

- Agent drives keys; you do not type into the card in v1.
- Full mouse / resize TUI APIs are not exposed.
- Sessions die when the Harness process exits.

## Tests

```bash
cd dsh-tool-rustiq && node --test
```
