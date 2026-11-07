# MCP server for minidsp

Lets an AI assistant such as Claude read and adjust your miniDSP through `minidspd`, and drive
Room EQ Wizard (REW) for measurement-based EQ. For example: *"Flatten OUT 1's EQ, I'm going to
measure it"*, or *"Load the filters REW just generated into the left and right woofers"*.

## Setup

Requires Node.js 22.18 or later (it runs the TypeScript source directly) and a running `minidspd`.

```sh
cd mcp && npm install
```

**Claude Code:**

```sh
claude mcp add minidsp -- node /absolute/path/to/minidsp-rs/mcp/src/server.ts
```

**Claude Desktop** (`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "minidsp": {
      "command": "node",
      "args": ["/absolute/path/to/minidsp-rs/mcp/src/server.ts"]
    }
  }
}
```

Environment variables, if the defaults don't fit:

| Variable | Default | |
|---|---|---|
| `MINIDSPD_URL` | `http://127.0.0.1:5380` | Where `minidspd` serves its API (e.g. the Pi's address) |
| `REW_API_URL` | `http://127.0.0.1:4735` | REW's API server |

## Tools

| Tool | Does |
|---|---|
| `minidsp_list_devices` | Lists connected devices |
| `minidsp_get_device` | Channels, filter sample rate, master status, levels, names, links, locks |
| `minidsp_set_master` | Volume, mute, active preset slot, source |
| `minidsp_set_channel` | Gain, mute, delay, polarity of a channel |
| `minidsp_set_peq` | Parametric EQ bands (designed at the device's sample rate) |
| `minidsp_flatten_peq` | Sets every PEQ band of a channel flat |
| `minidsp_load_rew_biquads` | Loads a REW "Save filter coefficients to file" export into a channel |
| `minidsp_set_crossover` | Butterworth or Linkwitz-Riley crossover on an output |
| `minidsp_set_compressor` | Output compressor settings |
| `minidsp_list_presets` / `minidsp_load_preset` | The preset library, and loading a preset into a slot |
| `rew_request` | Calls REW's API (measurements, EQ match, generated filters) |

Channels are numbered from 1, as in the web interface. Changes follow channel links set up in the
web interface, unless `follow_links` is false.

## Safety

- Raising the master volume by more than 6 dB at once is refused unless the assistant confirms
  with you first.
- Source and preset changes respect the locks set in the web interface; the daemon refuses them
  while locked.
- `rew_request` only talks to the configured REW server.
- Your MCP client asks before each tool call unless you've allowed it.

## REW workflow

1. In the web interface, unlock and select the preset slot you're working on.
2. Ask the assistant to flatten the channel's EQ (`minidsp_flatten_peq`) and mute the other outputs.
3. Measure in REW. With REW's API server enabled, the assistant can list measurements and run the
   EQ match through `rew_request`; otherwise do it in REW.
4. In REW's EQ window, choose **miniDSP** and the model matching the device's filter sample rate
   (Flex HTx: *Flex with Dirac Live*, 48 kHz; 2x4 HD: *2x4 HD*, 96 kHz). `minidsp_get_device`
   reports the device's rate. The wrong model puts every filter at the wrong frequency.
5. Export with **Save filter coefficients to file** and have the assistant load it with
   `minidsp_load_rew_biquads`, then re-measure to verify.

## Limitations

- miniDSP devices can't report per-channel settings, so the assistant can't read the current EQ.
- The web interface only shows values it set itself, so changes made here don't appear there.
- REW's API endpoints aren't wrapped one by one; the assistant explores them through
  `rew_request`.

## Tests

With `minidspd` running on the mock devices (`cargo run --bin minidspd -- --config gui/dev/minidspd.toml`):

```sh
npm test
```
