#!/bin/bash
set -e

# Creates an anchor, write out the shell command next to a prompt and execute it
cli_cmd() {
    NAME=$1
    shift
    CMD=$*

    echo "# ANCHOR: ${NAME}"
    echo "$ minidsp ${CMD}"
    minidsp "$@"
    echo "# ANCHOR_END: ${NAME}"
}

cli_cmd "help" "--help"

# Master Status
cli_cmd "config_help" config --help
cli_cmd "gain_help" gain --help
cli_cmd "mute_help" mute --help
cli_cmd "source_help" source --help
cli_cmd "dirac_help" dirac --help

# Input
cli_cmd "input_help" input --help
cli_cmd "input_gain_help" input 0 gain --help
cli_cmd "input_mute_help" input 0 mute --help
cli_cmd "input_peq_help" input 0 peq --help
cli_cmd "input_routing_help" input 0 routing --help

# Output
cli_cmd "output_help" output --help
cli_cmd "output_delay_help" output 0 delay --help
cli_cmd "output_fir_help" output 0 fir --help
cli_cmd "output_invert_help" output 0 invert --help
cli_cmd "output_crossover_help" output 0 crossover --help
cli_cmd "output_compressor_help" output 0 compressor --help
cli_cmd "output_mute_help" output 0 mute --help
cli_cmd "output_peq_help" output 0 peq --help
cli_cmd "output_gain_help" output 0 gain --help

