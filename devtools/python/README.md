# Python device-profile generator

Generates a minidsp-rs device profile (Rust source for `protocol/src/device/<name>.rs`) from the SigmaStudio XML shipped in the **device support package** of MiniDSP Device Console.

This is a **parallel path** to the Rust codegen in `devtools/src/codegen/`. The two tools consume different XML formats:

| | Rust codegen (`devtools/src/codegen/`) | Python codegen (this folder) |
|---|---|---|
| Input format | `<setting version="1.17">` (saved settings export) | `<XML><DSP_ID>N</DSP_ID>…</XML>` (device support package) |
| Source on disk | Exported via the official desktop app's "Save Settings" | `MiniDSP Device Console.app/Contents/Resources/device_support_package/DSP_Structures/<dsp_version>/` |
| Captures address mapping | Yes | Yes |
| Captures *default* parameter values | Yes (current settings) | Yes (factory defaults) |
| Requires a physical device to extract | Sometimes (some defaults differ from current) | No (extracted from the desktop app) |

The Python path was used to add the Flex 8 profile because the `<setting>` saved-export was not readily available for that device. Either path should work for future devices — pick whichever XML you already have.

## Workflow

```
DSP_default_parameters.xml ──┐
                             ├─► parse_minidsp_xml.py ─► symbols.json ─► generate_device_profile.py ─► flex8.rs
DSP_structure_layout.xml   ──┘
```

### Step 1 — Locate the XML files

On macOS, the device support package lives at:

```
/Applications/MiniDSP Device Console.app/Contents/Resources/device_support_package/DSP_Structures/<dsp_version>/
```

You'll find two files of interest:

- `DSP_default_parameters.xml` (~90 KB): the symbol → idx mapping (one entry per parameter, addresses in memory, default values)
- `DSP_structure_layout.xml` (~6 KB): pipeline topology, confirms which `chn` IDs are inputs vs outputs and what the per-channel signal chain looks like

The `dsp_version` directory name matches the value returned by `minidsp probe` (e.g. `110` for the Flex 8, `100` for the 2x4HD, etc.).

### Step 2 — Run the parser

```bash
python3 parse_minidsp_xml.py \
    --xml /path/to/DSP_default_parameters.xml \
    --output symbols.json \
    --input-chns 1,2 \
    --output-chns 3..10
```

The parser identifies symbols by naming convention (`DGain_X_0`, `PEQ_X_Y`, `BPF_X_Y`, `Delay_X_0`, etc.) and groups them per channel. Channel IDs come from the structure layout file: the convention seen on Flex 8 (and other recent MiniDSP devices) is that inputs use the low channel IDs (1, 2) and outputs start at 3.

If the parser's output looks wrong for your device, check `DSP_structure_layout.xml` to confirm the channel assignments — adjust `--input-chns` / `--output-chns` accordingly.

### Step 3 — Generate the Rust profile

```bash
python3 generate_device_profile.py \
    --symbols symbols.json \
    --output ../../protocol/src/device/<name>.rs \
    --product-name <Name> \
    --internal-sampling-rate 96000 \
    --fir-max-taps 4096 \
    --sources "Analog,Toslink,Spdif,Usb" \
    --omit-compressor
```

Then run `cargo fmt -p minidsp-protocol` to align formatting with the rest of the crate.

### Step 4 — Wire the profile in

Manual additions to existing files (see git diff in the Flex 8 PR for an example):

- `protocol/src/device/mod.rs` — add `#[cfg(feature = "device_<name>")] pub mod <name>;`
- `protocol/src/device/probe.rs` — add the variant + match arm `(hw_id, _) => <Name>` + by_kind dispatch
- `protocol/src/source.rs` — add the source-id mapping for the new hw_id
- `protocol/Cargo.toml` — declare the new feature + add it to `all_devices`

## Tested on

- miniDSP Flex 8 (hw_id=30, dsp_version=110, firmware 2.1) — produced the `flex8.rs` profile shipped in the same PR

## Limitations

- Naming conventions are heuristically detected. Older devices may use slightly different patterns (e.g. `D_GAIN_X_0` vs `DGain_X_0`) — adjust the regex classifiers in `parse_minidsp_xml.py` if a new device deviates.
- The script preserves the m2x4hd convention of listing PEQs in reverse order (`peq: &[PEQ_X_10, …, PEQ_X_1]`) — that matches what `apply_peq` expects in minidsp-rs.
- Compressor symbols are present in the `sym` module but `Output.compressor` is set to `None` when `--omit-compressor` is used. Wire it in a follow-up PR after live validation.
- FIR is assumed to live on inputs (see the Flex 8 case). If your device puts FIR on outputs, you'll need to extend the generator (or hand-edit the generated file).
