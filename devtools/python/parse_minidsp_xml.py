"""Parse a SigmaStudio ``DSP_default_parameters.xml`` and emit a structured
symbol table (JSON) for downstream consumption by ``generate_device_profile.py``.

This tool consumes the *device support package* XML files that ship with
MiniDSP Device Console (path on macOS:
``/Applications/MiniDSP Device Console.app/Contents/Resources/device_support_package/DSP_Structures/<dsp_version>/``).
It is an alternative to the Rust codegen in ``devtools/src/codegen/`` which
consumes the ``<setting version="1.17">`` "saved settings" XML format.

Tested on the miniDSP Flex 8 (DSP id 110). The class-of-symbols approach should
also work for siblings (Flex, FlexHtx, etc.) but adjust the channel mapping
and naming patterns if needed.

Usage::

    python3 parse_minidsp_xml.py \\
        --xml /path/to/DSP_default_parameters.xml \\
        --output symbols.json \\
        --input-chns 1,2 \\
        --output-chns 3..10
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path
from xml.etree import ElementTree as ET


def _parse_chn_arg(spec: str) -> list[int]:
    """Parse ``--input-chns`` / ``--output-chns`` args.

    Accepts comma list (``1,2``), dotted range (``3..10`` inclusive), or
    a mix (``1,2,5..8``).
    """
    out: list[int] = []
    for part in spec.split(","):
        part = part.strip()
        if ".." in part:
            lo, hi = part.split("..")
            out.extend(range(int(lo), int(hi) + 1))
        elif part:
            out.append(int(part))
    return out


def parse_items(defaults_path: Path) -> list[dict]:
    """Extract every ``<item ... idx="N">`` from the defaults XML.

    Returns a list of dicts with keys: name, idx, rep, range, value.
    """
    tree = ET.parse(defaults_path)
    items: list[dict] = []
    for item in tree.iter("item"):
        name = item.attrib.get("name")
        idx = item.attrib.get("idx")
        if name is None or idx is None:
            continue
        items.append(
            {
                "name": name,
                "idx": int(idx),
                "rep": item.attrib.get("rep"),
                "range": item.attrib.get("range"),
                "value_raw": (item.text or "").strip(),
            }
        )
    return items


def parse_blocks(defaults_path: Path) -> list[dict]:
    """Extract every typed block element with an ``idx`` attribute.

    Two flavours coexist:

    - ``<fir type="FIR" chn="N" cols=".." paraCnt=".." idx="N">`` with subpara
      rows for the bulk floating-point coefficients.
    - ``<filter type="PEQ|HPF|LPF" chn="N" band="B" idx="N">`` with 5 child
      ``<dec>`` coefficients (a single biquad).

    Both reserve a contiguous range of idx slots: 5 per biquad filter,
    paraCnt × cols-ish per FIR.
    """
    tree = ET.parse(defaults_path)
    blocks: list[dict] = []
    for tag in ("fir", "filter", "biquad", "delay", "compressor"):
        for el in tree.iter(tag):
            if "idx" not in el.attrib:
                continue
            blocks.append(
                {
                    "tag": tag,
                    "name": el.attrib.get("name"),
                    "type": el.attrib.get("type"),
                    "chn": int(el.attrib["chn"]) if "chn" in el.attrib else None,
                    "band": int(el.attrib["band"]) if "band" in el.attrib else None,
                    "cols": int(el.attrib["cols"]) if "cols" in el.attrib else None,
                    "paraCnt": int(el.attrib["paraCnt"]) if "paraCnt" in el.attrib else None,
                    "idx": int(el.attrib["idx"]),
                }
            )
    return blocks


def classify_filters(blocks: list[dict]) -> dict:
    """Group ``<filter ... type="..." chn="N" band="B" idx="...">`` by chn.

    Returns a dict ``{chn: {"PEQ": {band: idx, ...}, "HPF": {...}, "LPF": {...}}}``.
    """
    out: dict[int, dict[str, dict[int, int]]] = defaultdict(lambda: defaultdict(dict))
    for b in blocks:
        if b["tag"] != "filter":
            continue
        if b["chn"] is None or b["band"] is None or b["type"] is None:
            continue
        out[b["chn"]][b["type"]][b["band"]] = b["idx"]
    return {chn: dict(d) for chn, d in out.items()}


CHN_PAT = re.compile(r"^(?P<sym>[A-Za-z_]+(?:_[A-Za-z]+)*)_(?P<chn>\d+)(?:_(?P<band>\d+))?(?P<suffix>_status|_pol|_Taps|_Coeffs)?$")


def classify(items: list[dict]) -> dict:
    """Group items by their semantic role (gain, mute, peq, delay, …).

    Naming conventions in the Flex 8 XML follow the templates declared in
    ``DSP_structure_layout.xml``:

    - ``DGain_$C_0``            : output/input gain
    - ``DGain_$C_0_status``     : output/input mute (1 = muted, 2 = unmuted)
    - ``polarity_out_$C_0``     : output polarity (0 = positive, 1 = inverted)
    - ``Delay_$C_0``            : output delay
    - ``COMP_$C_0_status``      : compressor enable
    - ``PEQ_$C_$B``             : PEQ block (10 bands, $B = 0..9)
    - ``BPF_$C_$B``             : BPF block — bands 0..0 = LPF (1 band), 1..5 = HPF (5 bands)
    - ``FIR_$C_0_status``       : FIR bypass
    - ``FIR_$C_0_Taps``         : FIR active tap count
    - ``Mixer_$N_$M``           : input mixer 2x8
    - ``Mixer_$N_$M_status``    : per-cell mute
    - ``Mixer_$N_$M_pol``       : per-cell polarity
    - ``Meter_*``               : RMS meters (read-only)
    """
    by_chn: dict[int, dict] = defaultdict(lambda: {"peq": {}, "bpf": {}})
    mixer: dict[tuple[int, int], dict] = defaultdict(dict)
    misc: list[dict] = []

    # Patterns
    re_dgain = re.compile(r"^DGain_(\d+)_0(?P<suffix>_status)?$")
    re_polarity = re.compile(r"^polarity_out_(\d+)_0$")
    re_delay = re.compile(r"^Delay_(\d+)_0$")
    re_comp = re.compile(r"^COMP_(\d+)_0(?P<suffix>_status)?$")
    re_peq = re.compile(r"^PEQ_(\d+)_(\d+)(?P<suffix>_status)?$")
    re_bpf = re.compile(r"^BPF_(\d+)_(\d+)(?P<suffix>_status)?$")
    re_fir = re.compile(r"^FIR_(\d+)_0(?P<suffix>_status|_Taps|_Coeffs)?$")
    re_meter = re.compile(r"^Meter_(\d+)_0$")
    re_meter_named = re.compile(r"^Meter_(In|Out|Comp)_(\d+)$")
    re_mixer = re.compile(r"^Mixer_(\d+)_(\d+)(?P<suffix>_status|_pol)?$")
    re_simple_int = re.compile(r"^(Input|Output|Group)_")

    for it in items:
        name = it["name"]
        idx = it["idx"]

        m = re_dgain.match(name)
        if m:
            chn = int(m.group(1))
            field = "mute" if m.group("suffix") else "gain"
            by_chn[chn][field] = idx
            continue
        m = re_polarity.match(name)
        if m:
            by_chn[int(m.group(1))]["polarity"] = idx
            continue
        m = re_delay.match(name)
        if m:
            by_chn[int(m.group(1))]["delay"] = idx
            continue
        m = re_comp.match(name)
        if m:
            chn = int(m.group(1))
            field = "comp_status" if m.group("suffix") else "comp"
            by_chn[chn][field] = idx
            continue
        m = re_peq.match(name)
        if m:
            chn = int(m.group(1))
            band = int(m.group(2))
            field = "status" if m.group("suffix") else "biquad"
            by_chn[chn]["peq"].setdefault(band, {})[field] = idx
            continue
        m = re_bpf.match(name)
        if m:
            chn = int(m.group(1))
            band = int(m.group(2))
            field = "status" if m.group("suffix") else "biquad"
            by_chn[chn]["bpf"].setdefault(band, {})[field] = idx
            continue
        m = re_fir.match(name)
        if m:
            chn = int(m.group(1))
            sfx = m.group("suffix")
            field = {None: "fir", "_status": "fir_status", "_Taps": "fir_taps", "_Coeffs": "fir_coeffs"}[sfx]
            by_chn[chn][field] = idx
            continue
        m = re_meter.match(name)
        if m:
            by_chn[int(m.group(1))]["meter"] = idx
            continue
        m = re_meter_named.match(name)
        if m:
            # ignore — read-only RMS meters at the top of memory
            continue
        m = re_mixer.match(name)
        if m:
            n = int(m.group(1))
            mm = int(m.group(2))
            field = "gain"
            if m.group("suffix") == "_status":
                field = "status"
            elif m.group("suffix") == "_pol":
                field = "polarity"
            mixer[(n, mm)][field] = idx
            continue
        if re_simple_int.match(name):
            continue
        misc.append({"name": name, "idx": idx})

    return {"by_chn": dict(by_chn), "mixer": {f"{n}_{m}": v for (n, m), v in mixer.items()}, "misc": misc}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--xml", type=Path, required=True,
                        help="Path to DSP_default_parameters.xml from the device support package")
    parser.add_argument("--output", type=Path, default=Path("symbols.json"),
                        help="Output JSON file (default: ./symbols.json)")
    parser.add_argument("--input-chns", type=_parse_chn_arg, default="1,2",
                        help="Comma list or X..Y range of DSP channel ids that are inputs (default: 1,2)")
    parser.add_argument("--output-chns", type=_parse_chn_arg, default="3..10",
                        help="Comma list or X..Y range of DSP channel ids that are outputs (default: 3..10)")
    args = parser.parse_args()

    if not args.xml.exists():
        print(f"ERR: {args.xml} not found", file=sys.stderr)
        return 1

    input_chn_map = {chn: f"IN{i + 1}" for i, chn in enumerate(args.input_chns)}
    output_chn_map = {chn: f"OUT{i + 1}" for i, chn in enumerate(args.output_chns)}

    items = parse_items(args.xml)
    blocks = parse_blocks(args.xml)
    cls = classify(items)
    filters_by_chn = classify_filters(blocks)

    # Flat name → idx mapping (covers everything, including meters classified
    # as "read-only" earlier — the generator needs them by name)
    by_name = {it["name"]: it["idx"] for it in items}

    summary = {
        "total_items": len(items),
        "total_blocks": len(blocks),
        "input_channels": input_chn_map,
        "output_channels": output_chn_map,
        "by_chn": cls["by_chn"],
        "filters_by_chn": filters_by_chn,
        "mixer": cls["mixer"],
        "misc": cls["misc"],
        "by_name": by_name,
        "blocks": blocks,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(summary, indent=2, sort_keys=True, default=str))
    print(f"Wrote {args.output}  ({len(items)} items, {len(blocks)} blocks)")

    # Human-readable summary of the output channels
    print()
    print("=== Output channels (front-panel label ↔ chn) ===")
    print(
        f"{'OUT':4s} {'chn':4s} {'mute':>6s} {'gain':>6s} {'pol':>5s} {'delay':>6s} "
        f"{'peq[1]':>7s} {'peq[10]':>7s} {'lpf[1]':>7s} {'hpf[1..5]':>14s}"
    )
    for i, chn in enumerate(args.output_chns):
        info = cls["by_chn"].get(chn, {})
        flt = filters_by_chn.get(chn, {})
        peq = flt.get("PEQ", {})
        lpf = flt.get("LPF", {})
        hpf = flt.get("HPF", {})
        peq1 = peq.get(1, "-")
        peq10 = peq.get(10, "-")
        lpf1 = lpf.get(1, "-")
        hpf_first = hpf.get(1, "-")
        hpf_last = hpf.get(5, "-")
        print(
            f"OUT{i + 1:<2d} {chn:<4d} "
            f"{info.get('mute', '-'):>6} {info.get('gain', '-'):>6} "
            f"{info.get('polarity', '-'):>5} {info.get('delay', '-'):>6} "
            f"{peq1:>7} {peq10:>7} {lpf1:>7} {hpf_first}..{hpf_last}"
        )

    print()
    print("=== Input channels ===")
    for i, chn in enumerate(args.input_chns):
        info = cls["by_chn"].get(chn, {})
        flt = filters_by_chn.get(chn, {})
        peq = flt.get("PEQ", {})
        peq1 = peq.get(1, "-")
        peq10 = peq.get(10, "-")
        print(
            f"IN{i + 1:<2d} {chn:<4d} "
            f"mute={info.get('mute', '-')} gain={info.get('gain', '-')} "
            f"peq[1]={peq1} peq[10]={peq10} fir={info.get('fir', '-')}"
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
