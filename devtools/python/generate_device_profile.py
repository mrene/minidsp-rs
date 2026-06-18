"""Emit a minidsp-rs ``Device`` profile (Rust source) from a symbol table JSON
produced by ``parse_minidsp_xml.py``.

The output is functionally equivalent in shape to what ``minidsp-devtools
codegen`` produces for other devices — we just use a separate path because we
consume a different XML format (the *device support package* JSON rather than
the ``<setting version="1.17">`` saved-settings format).

Usage::

    python3 generate_device_profile.py \\
        --symbols symbols.json \\
        --output flex8.rs \\
        --product-name Flex8 \\
        --internal-sampling-rate 96000 \\
        --fir-max-taps 4096 \\
        --sources Analog,Toslink,Spdif,Usb \\
        --omit-compressor

For the miniDSP Flex 8, the channel mapping is inputs 1..2 and outputs 3..10
(the ``DSP_structure_layout.xml`` makes the input/output pipeline explicit).
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def emit_sym_const(name: str, idx: int) -> str:
    return f"    pub const {name}: u16 = {idx};"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--symbols", type=Path, required=True,
                        help="Path to symbols.json (output of parse_minidsp_xml.py)")
    parser.add_argument("--output", type=Path, default=Path("device_profile.rs"),
                        help="Output Rust source file")
    parser.add_argument("--product-name", required=True,
                        help='Quoted product name for the Device struct (e.g. "Flex8")')
    parser.add_argument("--internal-sampling-rate", type=int, required=True,
                        help="DSP internal sample rate in Hz (e.g. 96000)")
    parser.add_argument("--fir-max-taps", type=int, default=4096,
                        help="Maximum FIR taps (default: 4096)")
    parser.add_argument("--sources", default="Analog,Toslink,Spdif,Usb",
                        help="Comma-separated audio source enum variants in order (default: Analog,Toslink,Spdif,Usb)")
    parser.add_argument("--omit-compressor", action="store_true",
                        help="Emit Output.compressor = None even if the symbols contain compressor addresses (use this if the compressor path is not validated)")
    parser.add_argument("--input-chns", type=lambda s: [int(x) for x in s.split(",")], default=[1, 2],
                        help="Comma list of DSP channel ids that are inputs (default: 1,2)")
    parser.add_argument("--output-chns", type=lambda s: list(range(int(s.split('..')[0]), int(s.split('..')[1]) + 1)) if ".." in s else [int(x) for x in s.split(",")],
                        default=list(range(3, 11)),
                        help="DSP channel ids that are outputs, comma list or X..Y range (default: 3..10)")
    args = parser.parse_args()

    INPUT_CHNS = args.input_chns
    OUTPUT_CHNS = args.output_chns

    data = json.loads(args.symbols.read_text())
    by_chn = {int(k): v for k, v in data["by_chn"].items()}
    # JSON dict keys are strings — recursively coerce {chn → {type → {band → idx}}}
    filters_by_chn: dict[int, dict[str, dict[int, int]]] = {}
    for chn_str, types in data["filters_by_chn"].items():
        filters_by_chn[int(chn_str)] = {
            ftype: {int(band): idx for band, idx in bands.items()}
            for ftype, bands in types.items()
        }
    mixer = data["mixer"]

    # Build the sym table — exact name conventions of m2x4hd.rs:
    # - D_GAIN_<chn>_0_STATUS, D_GAIN_<chn>_0
    # - MIXER_<n>_<m>_STATUS, MIXER_<n>_<m> (no SMOOTHED suffix here; Flex 8
    #   names mixer cells `Mixer_N_M` whereas the 2x4HD uses
    #   `MIXER_NX_M_SMOOTHED_1_*`)
    # - PEQ_<chn>_<band>, BPF_<chn>_<band>
    # - DELAY_<chn>_0
    # - POLARITY_OUT_<idx_1based>_0  (1-based for outputs, matches XML
    #   `polarity_out_3_0` etc.)
    # - COMP_<chn>_0_STATUS/THRESHOLD/GAIN/RATIO/KNEE/ATIME/RTIME
    # - METER_OUT_<chn>, METER_COMP_<chn>
    # - FIR_<chn>_0_STATUS, FIR_<chn>_0_TAPS, FIR_<chn>_0

    sym_lines: list[str] = []

    # --- Top-of-table block: matches the order of the 2x4HD profile so
    # readers familiar with the existing code feel at home.
    # 1) D_GAIN STATUS for all channels (inputs + outputs) in chn order
    for chn in INPUT_CHNS + OUTPUT_CHNS:
        info = by_chn.get(chn, {})
        if "mute" in info:
            sym_lines.append(emit_sym_const(f"D_GAIN_{chn}_0_STATUS", info["mute"]))
    # 2) Mixer status
    for cell_key in sorted(mixer.keys(), key=lambda s: tuple(int(x) for x in s.split("_"))):
        cell = mixer[cell_key]
        n, m = cell_key.split("_")
        if "status" in cell:
            sym_lines.append(emit_sym_const(f"MIXER_{n}_{m}_STATUS", cell["status"]))
    # 3) FIR status on inputs (Flex 8 has FIR on inputs, not outputs)
    for chn in INPUT_CHNS:
        info = by_chn.get(chn, {})
        if "fir_status" in info:
            sym_lines.append(emit_sym_const(f"FIR_{chn}_0_STATUS", info["fir_status"]))
    # 4) Delay status (not present on Flex 8 XML — the delay value itself is
    #    a single float, no bypass toggle. Skip.)
    # 5) Compressor status (outputs only)
    for chn in OUTPUT_CHNS:
        info = by_chn.get(chn, {})
        if "comp_status" in info:
            sym_lines.append(emit_sym_const(f"COMP_{chn}_0_STATUS", info["comp_status"]))
    # 6) D_GAIN values for all channels
    for chn in INPUT_CHNS + OUTPUT_CHNS:
        info = by_chn.get(chn, {})
        if "gain" in info:
            sym_lines.append(emit_sym_const(f"D_GAIN_{chn}_0", info["gain"]))
    # 7) Mixer gains
    for cell_key in sorted(mixer.keys(), key=lambda s: tuple(int(x) for x in s.split("_"))):
        cell = mixer[cell_key]
        n, m = cell_key.split("_")
        if "gain" in cell:
            sym_lines.append(emit_sym_const(f"MIXER_{n}_{m}", cell["gain"]))
    # 8) Compressor parameters
    comp_fields = ("threshold", "gain", "ratio", "knee", "atime", "rtime")
    # Sub-indices follow the XML order: status=N, threshold=N+2, gain=N+3, ratio=N+4, knee=N+5, atime=N+6, rtime=N+7
    # but for clean code we look them up directly via the items table — too
    # bulky to redo here, instead derive from comp_status idx.
    # In the XML: comp_status idx N, comp_gain N+1 (D_GAIN), threshold N+3, gain N+4,
    # ratio N+5, knee N+6, atime N+7, rtime N+8. The simplest is to read the
    # specific items from the misc table.
    # Flat name → idx lookup. Prefer the new by_name (full) over the misc
    # subset (which excludes meters and a few other categories).
    misc_lookup = dict(data.get("by_name", {}))
    for m in data["misc"]:
        misc_lookup.setdefault(m["name"], m["idx"])
    for chn in OUTPUT_CHNS:
        for field in comp_fields:
            key = f"COMP_{chn}_0_{field}"
            if key in misc_lookup:
                sym_lines.append(emit_sym_const(key.upper(), misc_lookup[key]))
    # 9) Delay values (outputs only)
    for chn in OUTPUT_CHNS:
        info = by_chn.get(chn, {})
        if "delay" in info:
            sym_lines.append(emit_sym_const(f"DELAY_{chn}_0", info["delay"]))
    # 10) Meters (input + comp + output)
    for chn in INPUT_CHNS:
        key = f"Meter_In_{chn}"
        if key in misc_lookup:
            sym_lines.append(emit_sym_const(f"METER_IN_{chn}", misc_lookup[key]))
    for chn in OUTPUT_CHNS:
        key = f"Meter_Comp_{chn}"
        if key in misc_lookup:
            sym_lines.append(emit_sym_const(f"METER_COMP_{chn}", misc_lookup[key]))
    for chn in OUTPUT_CHNS:
        key = f"Meter_Out_{chn}"
        if key in misc_lookup:
            sym_lines.append(emit_sym_const(f"METER_OUT_{chn}", misc_lookup[key]))
    # 11) Polarities (input + output)
    for chn in INPUT_CHNS:
        key = f"polarity_in_{chn}_0"
        if key in misc_lookup:
            sym_lines.append(emit_sym_const(f"POLARITY_IN_{chn}_0", misc_lookup[key]))
    for chn in OUTPUT_CHNS:
        info = by_chn.get(chn, {})
        if "polarity" in info:
            # Front-panel 1-based for OUT 1..8
            front = chn - 2
            sym_lines.append(emit_sym_const(f"POLARITY_OUT_{front}_0", info["polarity"]))
    # 12) FIR coefficients on inputs (taps + data base)
    for chn in INPUT_CHNS:
        info = by_chn.get(chn, {})
        if "fir_taps" in info:
            sym_lines.append(emit_sym_const(f"FIR_{chn}_0_TAPS", info["fir_taps"]))
        if "fir" in info:
            sym_lines.append(emit_sym_const(f"FIR_{chn}_0", info["fir"]))
    # 13) PEQ banks (10 bands per channel)
    for chn in INPUT_CHNS + OUTPUT_CHNS:
        peq = filters_by_chn.get(chn, {}).get("PEQ", {})
        # Bands are 1-based in the XML (1..10)
        for band in range(1, 11):
            if band in peq:
                sym_lines.append(emit_sym_const(f"PEQ_{chn}_{band}", peq[band]))
    # 14) BPF cascades (5 bands per output: band 1 = LPF, bands 2..5 = HPF
    #     cascades). The minidsp-rs Crossover struct expects the base address
    #     of each "biquad group of 4 sequential biquads"; following the
    #     2x4HD convention we expose BPF_<chn>_1 and BPF_<chn>_5 — minidsp-rs
    #     reads 4 sequential biquads starting at each base.
    for chn in OUTPUT_CHNS:
        bpf = filters_by_chn.get(chn, {}).get("BPF", {})
        for band in range(1, 6):
            if band in bpf:
                sym_lines.append(emit_sym_const(f"BPF_{chn}_{band}", bpf[band]))

    # Build SYMBOLS array (every const exposed in `sym` module — used by
    # `minidsp dump-syms` for debugging)
    const_names = []
    for line in sym_lines:
        # extract "NAME" from "    pub const NAME: u16 = ...;"
        const_names.append(line.strip().split()[2].rstrip(":"))

    symbols_entries = ",\n        ".join(
        f'("{n}", {n})' for n in const_names
    )

    # Helper to format a list of PEQ slots in REVERSE order (matches m2x4hd
    # convention — minidsp-rs apply_peq writes from PEQ[N] down to PEQ[1]).
    # rustfmt breaks 10-element lists into "9 + 1" (it would otherwise exceed
    # ~100 chars). We pre-format to match — saves a `cargo fmt` pass downstream.
    def peq_list_for_chn(chn: int) -> str:
        all_bands = [f"PEQ_{chn}_{band}" for band in range(10, 0, -1)]
        first_nine = ", ".join(all_bands[:9])
        last_one = all_bands[9]
        return f"{first_nine},\n                {last_one}"

    def bpf_list_for_chn(chn: int) -> str:
        # Use band=1 and band=5 like m2x4hd — minidsp-rs reads 4 sequential
        # biquads from each base address. So this defines 2 crossover slots
        # of 4 biquads each.
        return f"BPF_{chn}_1, BPF_{chn}_5"

    # Build inputs section
    input_blocks = []
    for chn in INPUT_CHNS:
        info = by_chn[chn]
        # Routing: this input feeds outputs 0..7 (which are chn 3..10)
        # Mixer cell key in XML: Mixer_<input_zerobased>_<output_zerobased>
        # IN 1 (chn=1) → Mixer_0_* ; IN 2 (chn=2) → Mixer_1_*
        mix_n = chn - 1  # 0 or 1
        routing_entries = []
        for m in range(8):  # 8 outputs
            key = f"{mix_n}_{m}"
            cell = mixer.get(key, {})
            if "status" not in cell or "gain" not in cell:
                continue
            routing_entries.append(
                f"                Gate {{\n"
                f"                    enable: MIXER_{mix_n}_{m}_STATUS,\n"
                f"                    gain: Some(MIXER_{mix_n}_{m}),\n"
                f"                }}"
            )
        routing_block = ",\n".join(routing_entries)
        meter = f"Some(METER_IN_{chn})"
        input_blocks.append(
            f"        Input {{\n"
            f"            gate: Some(Gate {{\n"
            f"                enable: D_GAIN_{chn}_0_STATUS,\n"
            f"                gain: Some(D_GAIN_{chn}_0),\n"
            f"            }}),\n"
            f"            meter: {meter},\n"
            f"            routing: &[\n{routing_block},\n            ],\n"
            f"            peq: &[\n                {peq_list_for_chn(chn)},\n            ],\n"
            f"        }}"
        )

    # Build outputs section
    # NOTE: ``compressor`` deliberately omitted from the upstream PR scope —
    # we have the symbol mapping (COMP_<chn>_0_STATUS/THRESHOLD/RATIO/ATIME/
    # RTIME/METER_COMP_<chn> are still in the `sym` module for reference) but
    # we have not validated the compressor path end-to-end (no live test, no
    # GUI cross-check). A follow-up PR can wire it back in once tested.
    output_blocks = []
    for i, chn in enumerate(OUTPUT_CHNS):
        front = chn - 2  # 1..8
        out_block = (
            f"        Output {{\n"
            f"            gate: Gate {{\n"
            f"                enable: D_GAIN_{chn}_0_STATUS,\n"
            f"                gain: Some(D_GAIN_{chn}_0),\n"
            f"            }},\n"
            f"            meter: Some(METER_OUT_{chn}),\n"
            f"            delay_addr: Some(DELAY_{chn}_0),\n"
            f"            invert_addr: POLARITY_OUT_{front}_0,\n"
            f"            peq: &[\n                {peq_list_for_chn(chn)},\n            ],\n"
            f"            xover: Some(Crossover {{\n"
            f"                peqs: &[{bpf_list_for_chn(chn)}],\n"
            f"            }}),\n"
            f"            compressor: {'None' if args.omit_compressor else 'None  // TODO: wire when validated'},\n"
            f"            fir: None,\n"
            f"        }}"
        )
        output_blocks.append(out_block)

    inputs_text = ",\n".join(input_blocks)
    outputs_text = ",\n".join(output_blocks)

    sources_list = ", ".join(args.sources.split(","))

    rust = f"""//
// Generated by ``devtools/python/generate_device_profile.py`` from a
// SigmaStudio device-support-package XML (parsed by ``parse_minidsp_xml.py``).
// DO NOT EDIT — re-run the generator if the layout changes.
//
use super::*;
pub mod sym {{
    #[allow(dead_code)]
{chr(10).join(sym_lines)}
    #[cfg(feature = "symbols")]
    pub const SYMBOLS: &[(&str, u16)] = &[
        {symbols_entries},
    ];
}}
#[allow(unused_imports)]
use sym::*;
pub const DEVICE: Device = Device {{
    product_name: "{args.product_name}",
    sources: &[{sources_list}],
    inputs: &[
{inputs_text},
    ],
    outputs: &[
{outputs_text},
    ],
    fir_max_taps: {args.fir_max_taps},
    internal_sampling_rate: {args.internal_sampling_rate},
    dialect: Dialect {{
        addr_encoding: AddrEncoding::AddrLen3,
        float_encoding: FloatEncoding::Float32LE,
    }},
    #[cfg(feature = "symbols")]
    symbols: SYMBOLS,
}};
"""
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rust)
    print(f"Wrote {args.output}  ({len(sym_lines)} sym consts, {len(input_blocks)} inputs, {len(output_blocks)} outputs)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
