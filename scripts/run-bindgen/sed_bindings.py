#!/usr/bin/env python3
import argparse
import re
from collections import namedtuple

parser = argparse.ArgumentParser(
    description='Postprocess generated bindings.rs')
parser.add_argument('infile', type=argparse.FileType('r', encoding='utf-8'),
                    help='Input file')
parser.add_argument('outfile_bindings', type=argparse.FileType('w', encoding='utf-8'),
                    help='Output file')
parser.add_argument('--outfile_enum', type=argparse.FileType('w', encoding='utf-8'), required=False,
                    help='Output file for enums')

DERIVE_ATTRIBUTE = '#[derive('
DOC_ATTRIBUTE = '#[doc'

def capitalize_underscore_word(word):
    """Capitalizes names based on underscore
    >>> capitalize_underscore_word('my_enum')
    'MyEnum'
    >>> capitalize_underscore_word('MY_FIELD')
    'MyField'
    """
    return ''.join(map(lambda word: word.capitalize(), word.split('_')))


def capitalize_enum(enum_name):
    """Capitalizes names based on underscore
    >>> capitalize_enum('pub enum my_enum')
    'pub enum MyEnum'
    """
    return re.sub('(?:enum )(.*)', 
        lambda match: 'enum ' + capitalize_underscore_word(match[1]), enum_name)


def get_prefix(item, num_parts):
    """t
    >>> get_prefix('FOO_A1', 0)
    Traceback (most recent call last):
        ...
    ValueError: num_parts should be positive
    >>> get_prefix('FOO_A1', 1)
    'FOO_'
    >>> get_prefix('FOO_A1', 2)
    'FOO_A1'
    >>> get_prefix('FOO_BAR_A1', 2)
    'FOO_BAR_'
    """
    if num_parts <= 0:
        raise ValueError('num_parts should be positive')
    parts_begin = '_'.join(item.split('_')[:num_parts])
    return item[:len(parts_begin)+1]


def has_common_prefix_up_to_num_parts(items, num_parts):
    first_prefix = get_prefix(items[0], num_parts)
    for item in items:
        prefix = get_prefix(item, num_parts)
        if first_prefix != prefix:
            return False
        # Validate prefix, remaining partm ust start with a letter, not a number
        if not re.match('[a-zA-Z]', item[len(prefix):]):
            return False
    return first_prefix


def longest_common_prefix(items):
    """Find longest common prefix, splitted at underscores (_)

    >>> longest_common_prefix(['FOO_A1', 'FOO_B2'])
    'FOO_'
    >>> longest_common_prefix(['FOO_ON', 'FOO_OFF'])
    'FOO_'
    >>> longest_common_prefix(['FOO_BAR_A1', 'FOO_BAR_A2'])
    'FOO_BAR_'
    >>> longest_common_prefix(['FOO#BAR_A1', 'FOO#BAR_A2'])
    'FOO#BAR_'
    >>> longest_common_prefix(['CEC_VERSION_1', 'CEC_VERSION_2'])
    'CEC_'
    >>> longest_common_prefix(['FOO-BAR-A1', 'FOO-BAR-A2'])
    ''
    >>> longest_common_prefix(['CEC_CHANNEL_NUMBER_FORMAT_MASK', 'CEC_1_PART_CHANNEL_NUMBER'])
    ''
    """
    if not items:
        return []
    min_parts = min(map(len, map(lambda s: s.split('_'), items)))
    assert min_parts > 0
    for num_parts in range(min_parts, 0, -1):
        prefix = has_common_prefix_up_to_num_parts(items, num_parts)
        if prefix:
            return prefix
    # No common prefix found
    return ''

EnumInfo = namedtuple('Enuminfo', ['start_row', 'end_row', 'enum_variant_to_value', 'orig_enum_variant_to_new'])

if __name__ == '__main__':
    args = parser.parse_args()

    enums = {}
    with args.infile as f:
        lines = f.readlines()

    #
    # Find enums in bindings.rs
    #
    for enum_start, line in enumerate(lines):
        if not line.startswith('pub enum '):
            continue
        enum_name = line.split(' ')[2]
        orig_enum_names_variants = []
        for enum_end_relative, end_line in enumerate(lines[enum_start + 1:]):
            if end_line.strip().startswith('#') or end_line.strip().startswith('//'):
                continue
            if end_line.startswith('}'):
                enum_end = enum_start + enum_end_relative + 2
                for start_before in range(1, enum_start + 1, 1):
                    if not lines[enum_start - start_before].strip().startswith('#['):
                        break
                enum_start_with_attributes = enum_start - start_before + 1
                enum_prefix = longest_common_prefix(orig_enum_names_variants)

                # Clean up enum names
                # - remove prefix: <COMMON_ENUM_FIELD_PREFIX>_<FIELD> to <FIELD>
                # - TEST_VAL -> TestVal
                enum_primitive_names = ['libcec_sys::' + name for name in orig_enum_names_variants]
                new_variants_names = [capitalize_underscore_word(key.replace(enum_prefix, '')) for key in orig_enum_names_variants] # TODO: CapitalizeThis
                enum_variant_to_value = dict(zip(new_variants_names, enum_primitive_names))
                orig_enum_variant_to_new_variant = dict(zip(orig_enum_names_variants, enum_variant_to_value))
                enums[enum_name] = EnumInfo(enum_start_with_attributes, enum_end,  
                    enum_variant_to_value, orig_enum_variant_to_new_variant)
                break
            try:
                key, value = end_line.strip().split('=')
                key = key.strip()
                orig_enum_names_variants.append(key)
            except ValueError as e:
                raise ValueError('Could not parse ' + end_line) from e
        else:
            raise ValueError('No end found for enum {}'.format(enum_name))

    #
    # Do the rewrites (not very efficient, scans the file with each enum)
    #
    for enum_name, (enum_start, enum_end, enum_variant_to_value, orig_enum_variant_to_new_variant) in enums.items():
        orig_enum_variants = list(orig_enum_variant_to_new_variant)
        members_pattern = (r'\b' +
                           '|'.join(map(re.escape, orig_enum_variants)) + r'\b')
        members_pattern_ref = (r'\b(?:' + enum_name + '::)' +
                               '|'.join(map(re.escape, orig_enum_variants)) + r'\b')
        for row, line in enumerate(lines):
            if line.strip().startswith(DOC_ATTRIBUTE):
                # Replace docs: <COMMON_ENUM_FIELD_PREFIX>_<FIELD> to <ENUM_NAME>::<FIELD>
                lines[row] = re.sub(members_pattern,
                                    lambda match: '::'.join([enum_name, match[0].replace(enum_prefix, '')]), line)
            elif enum_start <= row < enum_end:
                if line.startswith(DERIVE_ATTRIBUTE):
                    # 1. Derive FromPrimitive (provided by num_derive) for enums to allow construction from primitive value
                    lines[row] = line.replace(DERIVE_ATTRIBUTE, DERIVE_ATTRIBUTE + 'FromPrimitive, ')
                elif '=' in line:
                    # Replace enum field value to refer to constant with the full name
                    key, _ = line.split(' = ', 1)
                    key = key.strip()
                    new_variant = orig_enum_variant_to_new_variant[key]
                    lines[row] = '    ' + ' = '.join([new_variant, enum_variant_to_value[new_variant]]) + ',\n'
            else:
                # Replace references to enum fields: <ENUM_NAME>::<COMMON_ENUM_FIELD_PREFIX>_<FIELD> to <ENUM_NAME>::<FIELD>
                lines[row] = re.sub(members_pattern_ref,
                                    lambda match: match[0].replace(enum_prefix, ''), line)

    with args.outfile_bindings as f:
        f.write(''.join(lines))

    if args.outfile_enum:
        with args.outfile_enum as f:
            f.write('use num_derive::FromPrimitive;')
            f.write('\n\n//\n')
            f.write('// Enums\n')
            f.write('//\n')
            for enum_name, (enum_start, enum_end, enum_prefix, enum_members) in enums.items():
                for line in lines[enum_start:enum_end]:
                    if ' enum ' in line:
                        f.write(capitalize_enum(line))
                    else:
                        f.write(line)
