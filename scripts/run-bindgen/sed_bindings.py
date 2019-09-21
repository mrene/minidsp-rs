#!/usr/bin/env python3
import argparse
import re

parser = argparse.ArgumentParser(description='Process some integers.')
parser.add_argument('infile', type=argparse.FileType('r', encoding='utf-8'),
                    help='Input file')
parser.add_argument('outfile', type=argparse.FileType('w', encoding='utf-8'),
                    help='Output file')


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
        enum_members = []
        for enum_end_relative, end_line in enumerate(lines[enum_start + 1:]):
            if end_line.strip().startswith('#') or end_line.strip().startswith('//'):
                continue
            if end_line.startswith('}'):
                enum_end = enum_start + enum_end_relative + 1
                enums[enum_name] = (enum_start, enum_end, longest_common_prefix(
                    enum_members), enum_members)
                break
            try:
                key, value = end_line.strip().split('=')
                key = key.strip()
                enum_members.append(key)
            except ValueError as e:
                raise ValueError('Could not parse ' + end_line) from e
        else:
            raise ValueError('No end found for enum {}'.format(enum_name))

    #
    # Do the rewrites
    #
    for enum_name, (enum_start, enum_end, enum_prefix, enum_members) in enums.items():
        if enum_prefix == '':
            continue
        members_pattern = (r'\b' +
                           '|'.join(map(re.escape, enum_members)) + r'\b')
        members_pattern_ref = (r'\b(?:' + enum_name + '::)' +
                               '|'.join(map(re.escape, enum_members)) + r'\b')
        for row, line in enumerate(lines):
            if enum_start <= row < enum_end:
                # Replace enum field names (declaration): <COMMON_ENUM_FIELD_PREFIX>_<FIELD> to <FIELD>
                lines[row] = re.sub(members_pattern,
                                    lambda match: match[0].replace(enum_prefix, ''), lines[row])
            if line.strip().startswith('#[doc'):
                # Replace docs: <COMMON_ENUM_FIELD_PREFIX>_<FIELD> to <ENUM_NAME>::<FIELD>
                lines[row] = re.sub(members_pattern,
                                    lambda match: '::'.join([enum_name, match[0].replace(enum_prefix, '')]), line)
            else:
                # Replace references to enum fields: <ENUM_NAME>::<COMMON_ENUM_FIELD_PREFIX>_<FIELD> to <ENUM_NAME>::<FIELD>
                lines[row] = re.sub(members_pattern_ref,
                                    lambda match: match[0].replace(enum_prefix, ''), line)

    with args.outfile as f:
        f.write(''.join(lines))
