#!/usr/bin/python

# Copyright Mozilla Foundation. See the COPYRIGHT
# file at the top-level directory of this distribution.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

import json
import subprocess
import sys
import os.path

if (not os.path.isfile("../encoding/encodings.json")) or (not os.path.isfile("../encoding/indexes.json")):
  sys.stderr.write("This script needs a clone of https://github.com/whatwg/encoding/ (preferably at revision 1d519bf8e5555cef64cf3a712485f41cd1a6a990 ) next to the encoding_rs directory.\n");
  sys.exit(-1)

if not os.path.isfile("../encoding_c/src/lib.rs"):
  sys.stderr.write("This script also writes the generated parts of the encoding_c crate and needs a clone of https://github.com/hsivonen/encoding_c next to the encoding_rs directory.\n");
  sys.exit(-1)

if not os.path.isfile("../codepage/src/lib.rs"):
  sys.stderr.write("This script also writes the generated parts of the codepage crate and needs a clone of https://github.com/hsivonen/codepage next to the encoding_rs directory.\n");
  sys.exit(-1)

def cmp_from_end(one, other):
  c = cmp(len(one), len(other))
  if c != 0:
    return c
  i = len(one) - 1
  while i >= 0:
    c = cmp(one[i], other[i])
    if c != 0:
      return c
    i -= 1
  return 0


class Label:
  def __init__(self, label, preferred):
    self.label = label
    self.preferred = preferred
  def __cmp__(self, other):
    return cmp_from_end(self.label, other.label)

class CodePage:
  def __init__(self, code_page, preferred):
    self.code_page = code_page
    self.preferred = preferred
  def __cmp__(self, other):
    return self.code_page, other.code_page

def static_u16_table(name, data):
  data_file.write('''pub static %s: [u16; %d] = [
  ''' % (name, len(data)))

  for i in xrange(len(data)):
    data_file.write('0x%04X,\n' % data[i])

  data_file.write('''];

  ''')

def static_u16_table_from_indexable(name, data, item, feature):
  data_file.write('''#[cfg(all(
    feature = "less-slow-%s",
    not(feature = "fast-%s")
))]
static %s: [u16; %d] = [
  ''' % (feature, feature, name, len(data)))

  for i in xrange(len(data)):
    data_file.write('0x%04X,\n' % data[i][item])

  data_file.write('''];

  ''')

def static_u8_pair_table_from_indexable(name, data, item, feature):
  data_file.write('''#[cfg(all(
    feature = "less-slow-%s",
    not(feature = "fast-%s")
))]
static %s: [[u8; 2]; %d] = [
  ''' % (feature, feature, name, len(data)))

  for i in xrange(len(data)):
    data_file.write('[0x%02X, 0x%02X],\n' % data[i][item])

  data_file.write('''];

  ''')

def static_u8_pair_table(name, data, feature):
  data_file.write('''#[cfg(feature = "%s")]
static %s: [[u8; 2]; %d] = [
  ''' % (feature, name, len(data)))

  for i in xrange(len(data)):
    pair = data[i]
    if not pair:
      pair = (0, 0)
    data_file.write('[0x%02X, 0x%02X],\n' % pair)

  data_file.write('''];

  ''')

preferred = []

dom = []

labels = []

data = json.load(open("../encoding/encodings.json", "r"))

indexes = json.load(open("../encoding/indexes.json", "r"))

single_byte = []

multi_byte = []

def to_camel_name(name):
  if name == u"iso-8859-8-i":
    return u"Iso8I"
  if name.startswith(u"iso-8859-"):
    return name.replace(u"iso-8859-", u"Iso")
  return name.title().replace(u"X-", u"").replace(u"-", u"").replace(u"_", u"")

def to_constant_name(name):
  return name.replace(u"-", u"_").upper()

def to_snake_name(name):
  return name.replace(u"-", u"_").lower()

def to_dom_name(name):
  return name

# Guestimate based on
# https://w3techs.com/technologies/overview/character_encoding/all
# whose methodology is known to be bogus, but the results are credible for
# this purpose. UTF-16LE lifted up due to prevalence on Windows and
# "ANSI codepages" prioritized.
encodings_by_code_page_frequency = [
  "UTF-8",    
  "UTF-16LE",
  "windows-1252",
  "windows-1251",
  "GBK",
  "Shift_JIS",
  "EUC-KR",
  "windows-1250",
  "windows-1256",
  "windows-1254",
  "Big5",
  "windows-874",
  "windows-1255",
  "windows-1253",
  "windows-1257",
  "windows-1258",
  "EUC-JP",
  "ISO-8859-2",
  "ISO-8859-15",
  "ISO-8859-7",
  "KOI8-R",
  "gb18030",
  "ISO-8859-5",
  "ISO-8859-8-I",
  "ISO-8859-4",
  "ISO-8859-6",
  "ISO-2022-JP",
  "KOI8-U",
  "ISO-8859-13",
  "ISO-8859-3",
  "UTF-16BE",
  "IBM866",
  "ISO-8859-10",
  "ISO-8859-8",
  "macintosh",
  "x-mac-cyrillic",
  "ISO-8859-14",
  "ISO-8859-16",
]

encodings_by_code_page = {
  932: "Shift_JIS",
  936: "GBK",
  949: "EUC-KR",
  950: "Big5",
  866: "IBM866",
  874: "windows-874",
  1200: "UTF-16LE",
  1201: "UTF-16BE",
  1250: "windows-1250",
  1251: "windows-1251",
  1252: "windows-1252",
  1253: "windows-1253",
  1254: "windows-1254",
  1255: "windows-1255",
  1256: "windows-1256",
  1257: "windows-1257",
  1258: "windows-1258",
  10000: "macintosh",
  10017: "x-mac-cyrillic",
  20866: "KOI8-R",
  20932: "EUC-JP",
  21866: "KOI8-U",
  28592: "ISO-8859-2",
  28593: "ISO-8859-3",
  28594: "ISO-8859-4",
  28595: "ISO-8859-5",
  28596: "ISO-8859-6",
  28597: "ISO-8859-7",
  28598: "ISO-8859-8",
  28600: "ISO-8859-10",
  28603: "ISO-8859-13",
  28604: "ISO-8859-14",
  28605: "ISO-8859-15",
  28606: "ISO-8859-16",
  38598: "ISO-8859-8-I",
  50221: "ISO-2022-JP",
  54936: "gb18030",
  65001: "UTF-8",
}

code_pages_by_encoding = {}

for code_page, encoding in encodings_by_code_page.iteritems():
  code_pages_by_encoding[encoding] = code_page

encoding_by_alias_code_page = {
  951: "Big5",
  10007: "x-mac-cyrillic",
  20936: "GBK",
  20949: "EUC-KR",
  21010: "UTF-16LE", # Undocumented; needed by calamine for Excel compat
  28591: "windows-1252",
  28599: "windows-1254",
  28601: "windows-874",
  50220: "ISO-2022-JP",
  50222: "ISO-2022-JP",
  50225: "replacement", # ISO-2022-KR
  50227: "replacement", # ISO-2022-CN
  51949: "EUC-JP",
  51936: "GBK",
  51949: "EUC-KR",
  52936: "replacement", # HZ
}

code_pages = []

for name in encodings_by_code_page_frequency:
  code_pages.append(code_pages_by_encoding[name])

encodings_by_code_page.update(encoding_by_alias_code_page)

temp_keys = encodings_by_code_page.keys()
temp_keys.sort()
for code_page in temp_keys:
  if not code_page in code_pages:
    code_pages.append(code_page)

# The position in the index (0 is the first index entry,
# i.e. byte value 0x80) that starts the longest run of
# consecutive code points. Must not be in the first
# quadrant. If the character to be encoded is not in this
# run, the part of the index after the run is searched
# forward. Then the part of the index from 32 to the start
# of the run. The first quadrant is searched last.
#
# If there is no obviously most useful longest run,
# the index here is just used to affect the search order.
start_of_longest_run_in_single_byte = {
  "IBM866": 96, # 0 would be longest, but we don't want to start in the first quadrant
  "windows-874": 33,
  "windows-1250": 92,
  "windows-1251": 64,
  "windows-1252": 32,
  "windows-1253": 83,
  "windows-1254": 95,
  "windows-1255": 96,
  "windows-1256": 65,
  "windows-1257": 95, # not actually longest
  "windows-1258": 95, # not actually longest
  "macintosh": 106, # useless
  "x-mac-cyrillic": 96,
  "KOI8-R": 64, # not actually longest
  "KOI8-U": 64, # not actually longest
  "ISO-8859-2": 95, # not actually longest
  "ISO-8859-3": 95, # not actually longest
  "ISO-8859-4": 95, # not actually longest
  "ISO-8859-5": 46,
  "ISO-8859-6": 65,
  "ISO-8859-7": 83,
  "ISO-8859-8": 96,
  "ISO-8859-10": 90, # not actually longest
  "ISO-8859-13": 95, # not actually longest
  "ISO-8859-14": 95,
  "ISO-8859-15": 63,
  "ISO-8859-16": 95, # not actually longest
}

#

for group in data:
  if group["heading"] == "Legacy single-byte encodings":
    single_byte = group["encodings"]
  else:
    multi_byte.extend(group["encodings"])
  for encoding in group["encodings"]:
    preferred.append(encoding["name"])
    for label in encoding["labels"]:
      labels.append(Label(label, encoding["name"]))

for name in preferred:
  dom.append(to_dom_name(name))

preferred.sort()
labels.sort()
dom.sort(cmp=cmp_from_end)

longest_label_length = 0
longest_name_length = 0
longest_label = None
longest_name = None

for name in preferred:
  if len(name) > longest_name_length:
    longest_name_length = len(name)
    longest_name = name

for label in labels:
  if len(label.label) > longest_label_length:
    longest_label_length = len(label.label)
    longest_label = label.label

def longest_run_for_single_byte(name):
  if name == u"ISO-8859-8-I":
    name = u"ISO-8859-8"
  index = indexes[name.lower()]
  run_byte_offset = start_of_longest_run_in_single_byte[name]
  run_bmp_offset = index[run_byte_offset]
  previous_code_point = run_bmp_offset
  run_length = 1
  while True:
    i = run_byte_offset + run_length
    if i == len(index):
      break
    code_point = index[i]
    if previous_code_point + 1 != code_point:
      break
    previous_code_point = code_point
    run_length += 1
  return (run_bmp_offset, run_byte_offset, run_length)

def is_single_byte(name):
  for encoding in single_byte:
    if name == encoding["name"]:
      return True
  return False

def read_non_generated(path):
  partially_generated_file = open(path, "r")
  full = partially_generated_file.read()
  partially_generated_file.close()

  generated_begin = "// BEGIN GENERATED CODE. PLEASE DO NOT EDIT."
  generated_end = "// END GENERATED CODE"

  generated_begin_index = full.find(generated_begin)
  if generated_begin_index < 0:
    sys.stderr.write("Can't find generated code start marker in %s. Exiting.\n" % path)
    sys.exit(-1)
  generated_end_index = full.find(generated_end)
  if generated_end_index < 0:
    sys.stderr.write("Can't find generated code end marker in %s. Exiting.\n" % path)
    sys.exit(-1)

  return (full[0:generated_begin_index + len(generated_begin)],
          full[generated_end_index:])

(lib_rs_begin, lib_rs_end) = read_non_generated("src/lib.rs")

label_file = open("src/lib.rs", "w")

label_file.write(lib_rs_begin)
label_file.write("""
// Instead, please regenerate using generate-encoding-data.py

const LONGEST_LABEL_LENGTH: usize = %d; // %s

""" % (longest_label_length, longest_label))

for name in preferred:
  variant = None
  if is_single_byte(name):
    (run_bmp_offset, run_byte_offset, run_length) = longest_run_for_single_byte(name)
    variant = "SingleByte(&data::SINGLE_BYTE_DATA.%s, 0x%04X, %d, %d)" % (to_snake_name(u"iso-8859-8" if name == u"ISO-8859-8-I" else name), run_bmp_offset, run_byte_offset, run_length)
  else:
    variant = to_camel_name(name)

  docfile = open("doc/%s.txt" % name, "r")
  doctext = docfile.read()
  docfile.close()

  label_file.write('''/// The initializer for the [%s](static.%s.html) encoding.
///
/// For use only for taking the address of this form when
/// Rust prohibits the use of the non-`_INIT` form directly,
/// such as in initializers of other `static`s. If in doubt,
/// use the corresponding non-`_INIT` reference-typed `static`.
///
/// This part of the public API will go away if Rust changes
/// to make the referent of `pub const FOO: &'static Encoding`
/// unique cross-crate or if Rust starts allowing static arrays
/// to be initialized with `pub static FOO: &'static Encoding`
/// items.
pub static %s_INIT: Encoding = Encoding {
    name: "%s",
    variant: VariantEncoding::%s,
};

/// The %s encoding.
///
%s///
/// This will change from `static` to `const` if Rust changes
/// to make the referent of `pub const FOO: &'static Encoding`
/// unique cross-crate, so don't take the address of this
/// `static`.
pub static %s: &'static Encoding = &%s_INIT;

''' % (to_dom_name(name), to_constant_name(name), to_constant_name(name), to_dom_name(name), variant, to_dom_name(name), doctext, to_constant_name(name), to_constant_name(name)))

label_file.write("""static LABELS_SORTED: [&'static str; %d] = [
""" % len(labels))

for label in labels:
  label_file.write('''"%s",\n''' % label.label)

label_file.write("""];

static ENCODINGS_IN_LABEL_SORT: [&'static Encoding; %d] = [
""" % len(labels))

for label in labels:
  label_file.write('''&%s_INIT,\n''' % to_constant_name(label.preferred))

label_file.write('''];

''')
label_file.write(lib_rs_end)
label_file.close()

label_test_file = open("src/test_labels_names.rs", "w")
label_test_file.write('''// Any copyright to the test code below this comment is dedicated to the
// Public Domain. http://creativecommons.org/publicdomain/zero/1.0/

// THIS IS A GENERATED FILE. PLEASE DO NOT EDIT.
// Instead, please regenerate using generate-encoding-data.py

use super::*;

#[test]
fn test_all_labels() {
''')

for label in labels:
  label_test_file.write('''assert_eq!(Encoding::for_label(b"%s"), Some(%s));\n''' % (label.label, to_constant_name(label.preferred)))

label_test_file.write('''}
''')
label_test_file.close()

def null_to_zero(code_point):
  if not code_point:
    code_point = 0
  return code_point

(data_rs_begin, data_rs_end) = read_non_generated("src/data.rs")

data_file = open("src/data.rs", "w")
data_file.write(data_rs_begin)
data_file.write('''
// Instead, please regenerate using generate-encoding-data.py

#[repr(align(64))] // Align to cache lines
pub struct SingleByteData {
''')

# Single-byte

for encoding in single_byte:
  name = encoding["name"]
  if name == u"ISO-8859-8-I":
    continue

  data_file.write('''    pub %s: [u16; 128],
''' % to_snake_name(name))

data_file.write('''}

pub static SINGLE_BYTE_DATA: SingleByteData = SingleByteData {
''')

for encoding in single_byte:
  name = encoding["name"]
  if name == u"ISO-8859-8-I":
    continue

  data_file.write('''    %s: [
''' % to_snake_name(name))

  for code_point in indexes[name.lower()]:
    data_file.write('0x%04X,\n' % null_to_zero(code_point))

  data_file.write('''],
''')

data_file.write('''};

''')

# Big5

index = indexes["big5"]

astralness = []
low_bits = []

for code_point in index[942:19782]:
  if code_point:
    astralness.append(1 if code_point > 0xFFFF else 0)
    low_bits.append(code_point & 0xFFFF)
  else:
    astralness.append(0)
    low_bits.append(0)

# pad length to multiple of 32
for j in xrange(32 - (len(astralness) % 32)):
  astralness.append(0)

data_file.write('''#[cfg_attr(feature = "cargo-clippy", allow(unreadable_literal))]
static BIG5_ASTRALNESS: [u32; %d] = [
''' % (len(astralness) / 32))

i = 0
while i < len(astralness):
  accu = 0
  for j in xrange(32):
    accu |= astralness[i + j] << j
  data_file.write('0x%08X,\n' % accu)
  i += 32

data_file.write('''];

''')

static_u16_table("BIG5_LOW_BITS", low_bits)

# Encoder table for Level 1 Hanzi
# Note: If we were OK with doubling this table, we
# could use a directly-indexable table instead...
level1_hanzi_index = index[5495:10896]
level1_hanzi_pairs = []
for i in xrange(len(level1_hanzi_index)):
  hanzi_lead = (i / 157) + 0xA4
  hanzi_trail = (i % 157)
  hanzi_trail += 0x40 if hanzi_trail < 0x3F else 0x62
  level1_hanzi_pairs.append((level1_hanzi_index[i], (hanzi_lead, hanzi_trail)))
level1_hanzi_pairs.append((0x4E5A, (0xC8, 0x7B)))
level1_hanzi_pairs.append((0x5202, (0xC8, 0x7D)))
level1_hanzi_pairs.append((0x9FB0, (0xC8, 0xA1)))
level1_hanzi_pairs.append((0x5188, (0xC8, 0xA2)))
level1_hanzi_pairs.append((0x9FB1, (0xC8, 0xA3)))
level1_hanzi_pairs.sort(key=lambda x: x[0])

static_u16_table_from_indexable("BIG5_LEVEL1_HANZI_CODE_POINTS", level1_hanzi_pairs, 0, "big5-hanzi-encode")
static_u8_pair_table_from_indexable("BIG5_LEVEL1_HANZI_BYTES", level1_hanzi_pairs, 1, "big5-hanzi-encode")

# Fast Unified Ideograph encode
big5_unified_ideograph_bytes = [None] * (0x9FCC - 0x4E00)
for row in xrange(0x7E - 0x20):
  for column in xrange(157):
    pointer = 5024 + column + (row * 157)
    code_point = index[pointer]
    if code_point and code_point >= 0x4E00 and code_point <= 0x9FCB:
      unified_offset = code_point - 0x4E00
      unified_lead = 0xA1 + row
      unified_trail = (0x40 if column < 0x3F else 0x62) + column
      if code_point == 0x5341 or code_point == 0x5345 or not big5_unified_ideograph_bytes[unified_offset]:
        big5_unified_ideograph_bytes[unified_offset] = (unified_lead, unified_trail)

static_u8_pair_table("BIG5_UNIFIED_IDEOGRAPH_BYTES", big5_unified_ideograph_bytes, "fast-big5-hanzi-encode")

# JIS0208

index = indexes["jis0208"]

# JIS 0208 Level 1 Kanji
static_u16_table("JIS0208_LEVEL1_KANJI", index[1410:4375])

# JIS 0208 Level 2 Kanji and Additional Kanji
static_u16_table("JIS0208_LEVEL2_AND_ADDITIONAL_KANJI", index[4418:7808])

# IBM Kanji
static_u16_table("IBM_KANJI", index[8272:8632])

# Check that the other instance is the same
if index[8272:8632] != index[10744:11104]:
  raise Error()

# JIS 0208 symbols (all non-Kanji, non-range items)
symbol_index = []
symbol_triples = []
pointers_to_scan = [
  (0, 188),
  (658, 691),
  (1159, 1221),
]
in_run = False
run_start_pointer = 0
run_start_array_index = 0
for (start, end) in pointers_to_scan:
  for i in range(start, end):
    code_point = index[i]
    if in_run:
      if code_point:
        symbol_index.append(code_point)
      else:
        symbol_triples.append(run_start_pointer)
        symbol_triples.append(i - run_start_pointer)
        symbol_triples.append(run_start_array_index)
        in_run = False
    else:
      if code_point:
        in_run = True
        run_start_pointer = i
        run_start_array_index = len(symbol_index)
        symbol_index.append(code_point)
  if in_run:
    symbol_triples.append(run_start_pointer)
    symbol_triples.append(end - run_start_pointer)
    symbol_triples.append(run_start_array_index)
    in_run = False
if in_run:
  raise Error()

# Now add manually the two overlapping slices of
# index from the NEC/IBM extensions.
run_start_array_index = len(symbol_index)
symbol_index.extend(index[10736:10744])
# Later
symbol_triples.append(10736)
symbol_triples.append(8)
symbol_triples.append(run_start_array_index)
# Earlier
symbol_triples.append(8644)
symbol_triples.append(4)
symbol_triples.append(run_start_array_index)

static_u16_table("JIS0208_SYMBOLS", symbol_index)
static_u16_table("JIS0208_SYMBOL_TRIPLES", symbol_triples)

# Write down the magic numbers needed when preferring the earlier case
data_file.write('''const IBM_SYMBOL_START: usize = %d;''' % (run_start_array_index + 1))
data_file.write('''const IBM_SYMBOL_END: usize = %d;''' % (run_start_array_index + 4))
data_file.write('''const IBM_SYMBOL_POINTER_START: usize = %d;''' % 8645)

# JIS 0208 ranges (excluding kana)
range_triples = []
pointers_to_scan = [
  (188, 281),
  (470, 657),
  (1128, 1159),
  (8634, 8644),
  (10716, 10736),
]
in_run = False
run_start_pointer = 0
run_start_code_point = 0
previous_code_point = 0
for (start, end) in pointers_to_scan:
  for i in range(start, end):
    code_point = index[i]
    if in_run:
      if code_point:
        if previous_code_point + 1 != code_point:
          range_triples.append(run_start_pointer)
          range_triples.append(i - run_start_pointer)
          range_triples.append(run_start_code_point)
          run_start_pointer = i
          run_start_code_point = code_point
        previous_code_point = code_point
      else:
          range_triples.append(run_start_pointer)
          range_triples.append(i - run_start_pointer)
          range_triples.append(run_start_code_point)
          run_start_pointer = 0
          run_start_code_point = 0
          previous_code_point = 0
          in_run = False
    else:
      if code_point:
        in_run = True
        run_start_pointer = i
        run_start_code_point = code_point
        previous_code_point = code_point
  if in_run:
    range_triples.append(run_start_pointer)
    range_triples.append(end - run_start_pointer)
    range_triples.append(run_start_code_point)
    run_start_pointer = 0
    run_start_code_point = 0
    previous_code_point = 0
    in_run = False
if in_run:
  raise Error()

static_u16_table("JIS0208_RANGE_TRIPLES", range_triples)

# Encoder table for Level 1 Kanji
# Note: If we were OK with 30 KB more footprint, we
# could use a directly-indexable table instead...
level1_kanji_index = index[1410:4375]
level1_kanji_pairs = []
for i in xrange(len(level1_kanji_index)):
  pointer = 1410 + i
  (lead, trail) = divmod(pointer, 188)
  lead += 0x81 if lead < 0x1F else 0xC1
  trail += 0x40 if trail < 0x3F else 0x41
  level1_kanji_pairs.append((level1_kanji_index[i], (lead, trail)))
level1_kanji_pairs.sort(key=lambda x: x[0])

static_u16_table_from_indexable("JIS0208_LEVEL1_KANJI_CODE_POINTS", level1_kanji_pairs, 0, "kanji-encode")
static_u8_pair_table_from_indexable("JIS0208_LEVEL1_KANJI_SHIFT_JIS_BYTES", level1_kanji_pairs, 1, "kanji-encode")

# Fast encoder table for Kanji
kanji_bytes = [None] * (0x9FA1 - 0x4E00)
for pointer in xrange(len(index)):
  code_point = index[pointer]
  if code_point and code_point >= 0x4E00 and code_point <= 0x9FA0:
    (lead, trail) = divmod(pointer, 188)
    lead += 0x81 if lead < 0x1F else 0xC1
    trail += 0x40 if trail < 0x3F else 0x41
    # unset the high bit of lead if IBM Kanji
    if pointer >= 8272:
      lead = lead & 0x7F
    kanji_bytes[code_point - 0x4E00] = (lead, trail)

static_u8_pair_table("JIS0208_KANJI_BYTES", kanji_bytes, "fast-kanji-encode")

# ISO-2022-JP half-width katakana

# index is still jis0208
half_width_index = indexes["iso-2022-jp-katakana"]

data_file.write('''pub static ISO_2022_JP_HALF_WIDTH_TRAIL: [u8; %d] = [
''' % len(half_width_index))

for i in xrange(len(half_width_index)):
  code_point = half_width_index[i]
  pointer = index.index(code_point)
  trail = pointer % 94 + 0x21
  data_file.write('0x%02X,\n' % trail)

data_file.write('''];

''')

# EUC-KR

index = indexes["euc-kr"]

# Unicode 1.1 Hangul above the old KS X 1001 block
# Compressed form takes 35% of uncompressed form
pointers = []
offsets = []
previous_code_point = 0
for row in xrange(0x20):
  for column in xrange(190):
    i = column + (row * 190)
    # Skip the gaps
    if (column >= 0x1A and column < 0x20) or (column >= 0x3A and column < 0x40):
      continue
    code_point = index[i]
    if previous_code_point > code_point:
      raise Error()
    if code_point - previous_code_point != 1:
      adjustment = 0
      if column >= 0x40:
        adjustment = 12
      elif column >= 0x20:
        adjustment = 6
      pointers.append(column - adjustment + (row * (190 - 12)))
      offsets.append(code_point)
    previous_code_point = code_point

static_u16_table("CP949_TOP_HANGUL_POINTERS", pointers)
static_u16_table("CP949_TOP_HANGUL_OFFSETS", offsets)

# Unicode 1.1 Hangul to the left of the old KS X 1001 block
pointers = []
offsets = []
previous_code_point = 0
for row in xrange(0x46 - 0x20):
  for column in xrange(190 - 94):
    i = 6080 + column + (row * 190)
    # Skip the gaps
    if (column >= 0x1A and column < 0x20) or (column >= 0x3A and column < 0x40):
      continue
    if i > 13127:
      # Exclude unassigned on partial last row
      break
    code_point = index[i]
    if previous_code_point > code_point:
      raise Error()
    if code_point - previous_code_point != 1:
      adjustment = 0
      if column >= 0x40:
        adjustment = 12
      elif column >= 0x20:
        adjustment = 6
      pointers.append(column - adjustment + (row * (190 - 94 - 12)))
      offsets.append(code_point)
    previous_code_point = code_point

static_u16_table("CP949_LEFT_HANGUL_POINTERS", pointers)
static_u16_table("CP949_LEFT_HANGUL_OFFSETS", offsets)

# KS X 1001 Hangul
hangul_index = []
previous_code_point = 0
for row in xrange(0x48 - 0x2F):
  for column in xrange(94):
    code_point = index[9026 + column + (row * 190)]
    if previous_code_point >= code_point:
      raise Error()
    hangul_index.append(code_point)
    previous_code_point = code_point

static_u16_table("KSX1001_HANGUL", hangul_index)

# KS X 1001 Hanja
hanja_index = []
for row in xrange(0x7D - 0x49):
  for column in xrange(94):
    hanja_index.append(index[13966 + column + (row * 190)])

static_u16_table("KSX1001_HANJA", hanja_index)

# KS X 1001 symbols
symbol_index = []
for i in range(6176, 6270):
  symbol_index.append(index[i])
for i in range(6366, 6437):
  symbol_index.append(index[i])

static_u16_table("KSX1001_SYMBOLS", symbol_index)

# KS X 1001 Uppercase Latin
subindex = []
for i in range(7506, 7521):
  subindex.append(null_to_zero(index[i]))

static_u16_table("KSX1001_UPPERCASE", subindex)

# KS X 1001 Lowercase Latin
subindex = []
for i in range(7696, 7712):
  subindex.append(index[i])

static_u16_table("KSX1001_LOWERCASE", subindex)

# KS X 1001 Box drawing
subindex = []
for i in range(7126, 7194):
  subindex.append(index[i])

static_u16_table("KSX1001_BOX", subindex)

# KS X 1001 other
pointers = []
offsets = []
previous_code_point = 0
for row in xrange(10):
  for column in xrange(94):
    i = 6556 + column + (row * 190)
    code_point = index[i]
    # Exclude ranges that were processed as lookup tables
    # or that contain unmapped cells by filling them with
    # ASCII. Upon encode, ASCII code points will
    # never appear as the search key.
    if (i >= 6946 and i <= 6950):
      code_point = i - 6946
    elif (i >= 6961 and i <= 6967):
      code_point = i - 6961
    elif (i >= 6992 and i <= 6999):
      code_point = i - 6992
    elif (i >= 7024 and i <= 7029):
      code_point = i - 7024
    elif (i >= 7126 and i <= 7219):
      code_point = i - 7126
    elif (i >= 7395 and i <= 7409):
      code_point = i - 7395
    elif (i >= 7506 and i <= 7521):
      code_point = i - 7506
    elif (i >= 7696 and i <= 7711):
      code_point = i - 7696
    elif (i >= 7969 and i <= 7979):
      code_point = i - 7969
    elif (i >= 8162 and i <= 8169):
      code_point = i - 8162
    elif (i >= 8299 and i <= 8313):
      code_point = i - 8299
    elif (i >= 8347 and i <= 8359):
      code_point = i - 8347
    if code_point - previous_code_point != 1:
      pointers.append(column + (row * 94))
      offsets.append(code_point)
    previous_code_point = code_point

static_u16_table("KSX1001_OTHER_POINTERS", pointers)
# Omit the last offset, because the end of the last line
# is unmapped, so we don't want to look at it.
static_u16_table("KSX1001_OTHER_UNSORTED_OFFSETS", offsets[:-1])

# Fast Hangul and Hanja encode
hangul_bytes = [None] * (0xD7A4 - 0xAC00)
hanja_unified_bytes = [None] * (0x9F9D - 0x4E00)
hanja_compatibility_bytes = [None] * (0xFA0C - 0xF900)
for row in xrange(0x7D):
  for column in xrange(190):
    pointer = column + (row * 190)
    code_point = index[pointer]
    if code_point:
      lead = 0x81 + row
      trail = 0x41 + column
      if code_point >= 0xAC00 and code_point < 0xD7A4:
        hangul_bytes[code_point - 0xAC00] = (lead, trail)
      elif code_point >= 0x4E00 and code_point < 0x9F9D:
        hanja_unified_bytes[code_point - 0x4E00] = (lead, trail)
      elif code_point >= 0xF900 and code_point < 0xFA0C:
        hanja_compatibility_bytes[code_point - 0xF900] = (lead, trail)

static_u8_pair_table("CP949_HANGUL_BYTES", hangul_bytes, "fast-hangul-encode")
static_u8_pair_table("KSX1001_UNIFIED_HANJA_BYTES", hanja_unified_bytes, "fast-hanja-encode")
static_u8_pair_table("KSX1001_COMPATIBILITY_HANJA_BYTES", hanja_compatibility_bytes, "fast-hanja-encode")

# JIS 0212

index = indexes["jis0212"]

# JIS 0212 Kanji
static_u16_table("JIS0212_KANJI", index[1410:7211])

# JIS 0212 accented (all non-Kanji, non-range items)
symbol_index = []
symbol_triples = []
pointers_to_scan = [
  (0, 596),
  (608, 644),
  (656, 1409),
]
in_run = False
run_start_pointer = 0
run_start_array_index = 0
for (start, end) in pointers_to_scan:
  for i in range(start, end):
    code_point = index[i]
    if in_run:
      if code_point:
        symbol_index.append(code_point)
      elif index[i + 1]:
        symbol_index.append(0)
      else:
        symbol_triples.append(run_start_pointer)
        symbol_triples.append(i - run_start_pointer)
        symbol_triples.append(run_start_array_index)
        in_run = False
    else:
      if code_point:
        in_run = True
        run_start_pointer = i
        run_start_array_index = len(symbol_index)
        symbol_index.append(code_point)
  if in_run:
    symbol_triples.append(run_start_pointer)
    symbol_triples.append(end - run_start_pointer)
    symbol_triples.append(run_start_array_index)
    in_run = False
if in_run:
  raise Error()

static_u16_table("JIS0212_ACCENTED", symbol_index)
static_u16_table("JIS0212_ACCENTED_TRIPLES", symbol_triples)

# gb18030

index = indexes["gb18030"]

# Unicode 1.1 ideographs above the old GB2312 block
# Compressed form takes 63% of uncompressed form
pointers = []
offsets = []
previous_code_point = 0
for i in xrange(6080):
  code_point = index[i]
  if previous_code_point > code_point:
    raise Error()
  if code_point - previous_code_point != 1:
    pointers.append(i)
    offsets.append(code_point)
  previous_code_point = code_point

static_u16_table("GBK_TOP_IDEOGRAPH_POINTERS", pointers)
static_u16_table("GBK_TOP_IDEOGRAPH_OFFSETS", offsets)

# Unicode 1.1 ideographs to the left of the old GB2312 block
# Compressed form takes 40% of uncompressed form
pointers = []
offsets = []
previous_code_point = 0
for row in xrange(0x7D - 0x29):
  for column in xrange(190 - 94):
    i = 7790 + column + (row * 190)
    if i > 23650:
      # Exclude compatibility ideographs at the end
      break
    code_point = index[i]
    if previous_code_point > code_point:
      raise Error()
    if code_point - previous_code_point != 1:
      pointers.append(column + (row * (190 - 94)))
      offsets.append(code_point)
    previous_code_point = code_point

static_u16_table("GBK_LEFT_IDEOGRAPH_POINTERS", pointers)
static_u16_table("GBK_LEFT_IDEOGRAPH_OFFSETS", offsets)

# GBK other (excl. Ext A, Compat & PUA at the bottom)
pointers = []
offsets = []
previous_code_point = 0
for row in xrange(0x29 - 0x20):
  for column in xrange(190 - 94):
    i = 6080 + column + (row * 190)
    code_point = index[i]
    if code_point - previous_code_point != 1:
      pointers.append(column + (row * (190 - 94)))
      offsets.append(code_point)
    previous_code_point = code_point

pointers.append((190 - 94) * (0x29 - 0x20))
static_u16_table("GBK_OTHER_POINTERS", pointers)
static_u16_table("GBK_OTHER_UNSORTED_OFFSETS", offsets)

# GBK bottom: Compatibility ideagraphs, Ext A and PUA
bottom_index = []
# 5 compat following Unified Ideographs
for i in range(23651, 23656):
  bottom_index.append(index[i])
# Last row
for i in range(23750, 23846):
  bottom_index.append(index[i])

static_u16_table("GBK_BOTTOM", bottom_index)

# GB2312 Hanzi
# (and the 5 PUA code points in between Level 1 and Level 2)
hanzi_index = []
for row in xrange(0x77 - 0x2F):
  for column in xrange(94):
    hanzi_index.append(index[9026 + column + (row * 190)])

static_u16_table("GB2312_HANZI", hanzi_index)

# GB2312 symbols
symbol_index = []
for i in xrange(94):
  symbol_index.append(index[6176 + i])

static_u16_table("GB2312_SYMBOLS", symbol_index)

# GB2312 symbols on Greek row (incl. PUA)
symbol_index = []
for i in xrange(22):
  symbol_index.append(index[7189 + i])

static_u16_table("GB2312_SYMBOLS_AFTER_GREEK", symbol_index)

# GB2312 Pinyin
pinyin_index = []
for i in xrange(32):
  pinyin_index.append(index[7506 + i])

static_u16_table("GB2312_PINYIN", pinyin_index)

# GB2312 other (excl. bottom PUA)
pointers = []
offsets = []
previous_code_point = 0
for row in xrange(14):
  for column in xrange(94):
    i = 6366 + column + (row * 190)
    code_point = index[i]
    # Exclude the two ranges that were processed as
    # lookup tables above by filling them with
    # ASCII. Upon encode, ASCII code points will
    # never appear as the search key.
    if (i >= 7189 and i < 7189 + 22):
      code_point = i - 7189
    elif (i >= 7506 and i < 7506 + 32):
      code_point = i - 7506
    if code_point - previous_code_point != 1:
      pointers.append(column + (row * 94))
      offsets.append(code_point)
    previous_code_point = code_point

pointers.append(14 * 94)
static_u16_table("GB2312_OTHER_POINTERS", pointers)
static_u16_table("GB2312_OTHER_UNSORTED_OFFSETS", offsets)

# Non-gbk code points
pointers = []
offsets = []
for pair in indexes["gb18030-ranges"]:
  if pair[1] == 0x10000:
    break # the last entry doesn't fit in u16
  pointers.append(pair[0])
  offsets.append(pair[1])

static_u16_table("GB18030_RANGE_POINTERS", pointers)
static_u16_table("GB18030_RANGE_OFFSETS", offsets)

# Encoder table for Level 1 Hanzi
# The units here really fit into 12 bits, but since we're
# looking for speed here, let's use 16 bits per unit.
# Once we use 16 bits per unit, we might as well precompute
# the output bytes.
level1_hanzi_index = hanzi_index[:(94 * (0xD8 - 0xB0) - 5)]
level1_hanzi_pairs = []
for i in xrange(len(level1_hanzi_index)):
  hanzi_lead = (i / 94) + 0xB0
  hanzi_trail = (i % 94) + 0xA1
  level1_hanzi_pairs.append((level1_hanzi_index[i], (hanzi_lead, hanzi_trail)))
level1_hanzi_pairs.sort(key=lambda x: x[0])

static_u16_table_from_indexable("GB2312_LEVEL1_HANZI_CODE_POINTS", level1_hanzi_pairs, 0, "gb-hanzi-encode")
static_u8_pair_table_from_indexable("GB2312_LEVEL1_HANZI_BYTES", level1_hanzi_pairs, 1, "gb-hanzi-encode")

# Fast Hanzi encoder table
hanzi_bytes = [None] * (0x9FA7 - 0x4E00)
for row in xrange(126):
  for column in xrange(190):
    pointer = column + (row * 190)
    code_point = index[pointer]
    if code_point and code_point >= 0x4E00 and code_point <= 0x9FA6:
      hanzi_lead = 0x81 + row
      hanzi_trail = column + (0x40 if column < 0x3F else 0x41)
      hanzi_bytes[code_point - 0x4E00] = (hanzi_lead, hanzi_trail)

static_u8_pair_table("GBK_HANZI_BYTES", hanzi_bytes, "fast-gb-hanzi-encode")

data_file.write(data_rs_end)

data_file.close()

# Variant

variant_file = open("src/variant.rs", "w")
variant_file.write('''// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// THIS IS A GENERATED FILE. PLEASE DO NOT EDIT.
// Instead, please regenerate using generate-encoding-data.py

//! This module provides enums that wrap the various decoders and encoders.
//! The purpose is to make `Decoder` and `Encoder` `Sized` by writing the
//! dispatch explicitly for a finite set of specialized decoders and encoders.
//! Unfortunately, this means the compiler doesn't generate the dispatch code
//! and it has to be written here instead.
//!
//! The purpose of making `Decoder` and `Encoder` `Sized` is to allow stack
//! allocation in Rust code, including the convenience methods on `Encoding`.

''')

encoding_variants = [u"single-byte",]
for encoding in multi_byte:
  if encoding["name"] in [u"UTF-16LE", u"UTF-16BE"]:
    continue
  else:
    encoding_variants.append(encoding["name"])
encoding_variants.append(u"UTF-16")

decoder_variants = []
for variant in encoding_variants:
  if variant == u"GBK":
    continue
  decoder_variants.append(variant)

encoder_variants = []
for variant in encoding_variants:
  if variant in [u"replacement", u"GBK", u"UTF-16"]:
    continue
  encoder_variants.append(variant)

for variant in decoder_variants:
  variant_file.write("use %s::*;\n" % to_snake_name(variant))

variant_file.write('''use super::*;

pub enum VariantDecoder {
''')

for variant in decoder_variants:
  variant_file.write("   %s(%sDecoder),\n" % (to_camel_name(variant), to_camel_name(variant)))

variant_file.write('''}

impl VariantDecoder {
''')

def write_variant_method(name, mut, arg_list, ret, variants, excludes, kind):
  variant_file.write('''pub fn %s(&''' % name)
  if mut:
    variant_file.write('''mut ''')
  variant_file.write('''self''')
  for arg in arg_list:
    variant_file.write(''', %s: %s''' % (arg[0], arg[1]))
  variant_file.write(''')''')
  if ret:
    variant_file.write(''' -> %s''' % ret)
  variant_file.write(''' {\nmatch *self {\n''')
  for variant in variants:
    variant_file.write('''Variant%s::%s(ref ''' % (kind, to_camel_name(variant)))
    if mut:
      variant_file.write('''mut ''')
    if variant in excludes:
      variant_file.write('''v) => (),''')
      continue
    variant_file.write('''v) => v.%s(''' % name)
    first = True
    for arg in arg_list:
      if not first:
        variant_file.write(''', ''')
      first = False
      variant_file.write(arg[0])
    variant_file.write('''),\n''')
  variant_file.write('''}\n}\n\n''')

write_variant_method("max_utf16_buffer_length", False, [("byte_length", "usize")], "Option<usize>", decoder_variants, [], "Decoder")

write_variant_method("max_utf8_buffer_length_without_replacement", False, [("byte_length", "usize")], "Option<usize>", decoder_variants, [], "Decoder")

write_variant_method("max_utf8_buffer_length", False, [("byte_length", "usize")], "Option<usize>", decoder_variants, [], "Decoder")

write_variant_method("decode_to_utf16_raw", True, [("src", "&[u8]"),
                           ("dst", "&mut [u16]"),
                           ("last", "bool")], "(DecoderResult, usize, usize)", decoder_variants, [], "Decoder")

write_variant_method("decode_to_utf8_raw", True, [("src", "&[u8]"),
                           ("dst", "&mut [u8]"),
                           ("last", "bool")], "(DecoderResult, usize, usize)", decoder_variants, [], "Decoder")

variant_file.write('''

    pub fn latin1_byte_compatible_up_to(&self, buffer: &[u8]) -> Option<usize> {
        match *self {
            VariantDecoder::SingleByte(ref v) => {
                return Some(v.latin1_byte_compatible_up_to(buffer));
            }
            VariantDecoder::Utf8(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::Gb18030(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::Big5(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::EucJp(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::Iso2022Jp(ref v) => {
                if v.in_neutral_state() {
                    return Some(Encoding::iso_2022_jp_ascii_valid_up_to(buffer));
                }
                return None;
            }
            VariantDecoder::ShiftJis(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::EucKr(ref v) => {
                if !v.in_neutral_state() {
                    return None;
                }
            }
            VariantDecoder::UserDefined(_) => {}
            VariantDecoder::Replacement(_) | VariantDecoder::Utf16(_) => {
                return None;
            }
        };
        Some(Encoding::ascii_valid_up_to(buffer))
    }
}

pub enum VariantEncoder {
''')

for variant in encoder_variants:
  variant_file.write("   %s(%sEncoder),\n" % (to_camel_name(variant), to_camel_name(variant)))

variant_file.write('''}

impl VariantEncoder {
    pub fn has_pending_state(&self) -> bool {
        match *self {
            VariantEncoder::Iso2022Jp(ref v) => {
                v.has_pending_state()
            }
            _ => false,
        }
    }
''')

write_variant_method("max_buffer_length_from_utf16_without_replacement", False, [("u16_length", "usize")], "Option<usize>", encoder_variants, [], "Encoder")

write_variant_method("max_buffer_length_from_utf8_without_replacement", False, [("byte_length", "usize")], "Option<usize>", encoder_variants, [], "Encoder")

write_variant_method("encode_from_utf16_raw", True, [("src", "&[u16]"),
                           ("dst", "&mut [u8]"),
                           ("last", "bool")], "(EncoderResult, usize, usize)", encoder_variants, [], "Encoder")

write_variant_method("encode_from_utf8_raw", True, [("src", "&str"),
                           ("dst", "&mut [u8]"),
                           ("last", "bool")], "(EncoderResult, usize, usize)", encoder_variants, [], "Encoder")


variant_file.write('''}

pub enum VariantEncoding {
    SingleByte(&'static [u16; 128], u16, u8, u8),''')

for encoding in multi_byte:
  variant_file.write("%s,\n" % to_camel_name(encoding["name"]))

variant_file.write('''}

impl VariantEncoding {
    pub fn new_variant_decoder(&self) -> VariantDecoder {
        match *self {
            VariantEncoding::SingleByte(table, _, _, _) => SingleByteDecoder::new(table),
            VariantEncoding::Utf8 => Utf8Decoder::new(),
            VariantEncoding::Gbk | VariantEncoding::Gb18030 => Gb18030Decoder::new(),
            VariantEncoding::Big5 => Big5Decoder::new(),
            VariantEncoding::EucJp => EucJpDecoder::new(),
            VariantEncoding::Iso2022Jp => Iso2022JpDecoder::new(),
            VariantEncoding::ShiftJis => ShiftJisDecoder::new(),
            VariantEncoding::EucKr => EucKrDecoder::new(),
            VariantEncoding::Replacement => ReplacementDecoder::new(),
            VariantEncoding::UserDefined => UserDefinedDecoder::new(),
            VariantEncoding::Utf16Be => Utf16Decoder::new(true),
            VariantEncoding::Utf16Le => Utf16Decoder::new(false),
        }
    }

    pub fn new_encoder(&self, encoding: &'static Encoding) -> Encoder {
        match *self {
            VariantEncoding::SingleByte(table, run_bmp_offset, run_byte_offset, run_length) => SingleByteEncoder::new(encoding, table, run_bmp_offset, run_byte_offset, run_length),
            VariantEncoding::Utf8 => Utf8Encoder::new(encoding),
            VariantEncoding::Gbk => Gb18030Encoder::new(encoding, false),
            VariantEncoding::Gb18030 => Gb18030Encoder::new(encoding, true),
            VariantEncoding::Big5 => Big5Encoder::new(encoding),
            VariantEncoding::EucJp => EucJpEncoder::new(encoding),
            VariantEncoding::Iso2022Jp => Iso2022JpEncoder::new(encoding),
            VariantEncoding::ShiftJis => ShiftJisEncoder::new(encoding),
            VariantEncoding::EucKr => EucKrEncoder::new(encoding),
            VariantEncoding::UserDefined => UserDefinedEncoder::new(encoding),
            VariantEncoding::Utf16Be | VariantEncoding::Replacement |
            VariantEncoding::Utf16Le => unreachable!(),
        }
    }

    pub fn is_single_byte(&self) -> bool {
        match *self {
            VariantEncoding::SingleByte(_, _, _, _) | VariantEncoding::UserDefined => true,
            _ => false,
        }
    }
}
''')

variant_file.close()

(ffi_rs_begin, ffi_rs_end) = read_non_generated("../encoding_c/src/lib.rs")

ffi_file = open("../encoding_c/src/lib.rs", "w")

ffi_file.write(ffi_rs_begin)
ffi_file.write("""
// Instead, please regenerate using generate-encoding-data.py

/// The minimum length of buffers that may be passed to `encoding_name()`.
pub const ENCODING_NAME_MAX_LENGTH: usize = %d; // %s

""" % (longest_name_length, longest_name))

for name in preferred:
  ffi_file.write('''/// The %s encoding.
#[no_mangle]
pub static %s_ENCODING: ConstEncoding = ConstEncoding(&%s_INIT);

''' % (to_dom_name(name), to_constant_name(name), to_constant_name(name)))

ffi_file.write(ffi_rs_end)
ffi_file.close()

(single_byte_rs_begin, single_byte_rs_end) = read_non_generated("src/single_byte.rs")

single_byte_file = open("src/single_byte.rs", "w")

single_byte_file.write(single_byte_rs_begin)
single_byte_file.write("""
// Instead, please regenerate using generate-encoding-data.py

    #[test]
    fn test_single_byte_decode() {""")

idx = 0 # for Miri, return after 2nd test
for name in preferred:
  if name == u"ISO-8859-8-I":
    continue;
  if is_single_byte(name):
    single_byte_file.write("""
        decode_single_byte(%s, &data::SINGLE_BYTE_DATA.%s);""" % (to_constant_name(name), to_snake_name(name)))
    idx += 1
    if idx == 2:
      single_byte_file.write("""
        if cfg!(miri) {
            // Miri is too slow
            return;
        }""")

single_byte_file.write("""
    }

    #[test]
    fn test_single_byte_encode() {""")


idx = 0 # for Miri, return after 2nd test
for name in preferred:
  if name == u"ISO-8859-8-I":
    continue;
  if is_single_byte(name):
    single_byte_file.write("""
        encode_single_byte(%s, &data::SINGLE_BYTE_DATA.%s);""" % (to_constant_name(name), to_snake_name(name)))
    idx += 1
    if idx == 2:
      single_byte_file.write("""
        if cfg!(miri) {
            // Miri is too slow
            return;
        }""")


single_byte_file.write("""
    }
""")

single_byte_file.write(single_byte_rs_end)
single_byte_file.close()

static_file = open("../encoding_c/include/encoding_rs_statics.h", "w")

static_file.write("""// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// THIS IS A GENERATED FILE. PLEASE DO NOT EDIT.
// Instead, please regenerate using generate-encoding-data.py

// This file is not meant to be included directly. Instead, encoding_rs.h
// includes this file.

#ifndef encoding_rs_statics_h_
#define encoding_rs_statics_h_

#ifndef ENCODING_RS_ENCODING
#define ENCODING_RS_ENCODING Encoding
#ifndef __cplusplus
typedef struct Encoding_ Encoding;
#endif
#endif

#ifndef ENCODING_RS_NOT_NULL_CONST_ENCODING_PTR
#define ENCODING_RS_NOT_NULL_CONST_ENCODING_PTR const ENCODING_RS_ENCODING*
#endif

#ifndef ENCODING_RS_ENCODER
#define ENCODING_RS_ENCODER Encoder
#ifndef __cplusplus
typedef struct Encoder_ Encoder;
#endif
#endif

#ifndef ENCODING_RS_DECODER
#define ENCODING_RS_DECODER Decoder
#ifndef __cplusplus
typedef struct Decoder_ Decoder;
#endif
#endif

#define INPUT_EMPTY 0

#define OUTPUT_FULL 0xFFFFFFFF

// %s
#define ENCODING_NAME_MAX_LENGTH %d

""" % (longest_name, longest_name_length))

for name in preferred:
  static_file.write('''/// The %s encoding.
extern ENCODING_RS_NOT_NULL_CONST_ENCODING_PTR const %s_ENCODING;

''' % (to_dom_name(name), to_constant_name(name)))

static_file.write("""#endif // encoding_rs_statics_h_
""")
static_file.close()

(utf_8_rs_begin, utf_8_rs_end) = read_non_generated("src/utf_8.rs")

utf_8_file = open("src/utf_8.rs", "w")

utf_8_file.write(utf_8_rs_begin)
utf_8_file.write("""
// Instead, please regenerate using generate-encoding-data.py

pub static UTF8_DATA: Utf8Data = Utf8Data {
    table: [
""")

for i in range(256):
  combined = (1 << 2) # invalid lead
  if i < 0x80 or i > 0xBF:
    combined |= (1 << 3) # normal trail
  if i < 0xA0 or i > 0xBF:
    combined |= (1 << 4) # three-byte special lower bound
  if i < 0x80 or i > 0x9F:
    combined |= (1 << 5) # three-byte special upper bound
  if i < 0x90 or i > 0xBF:
    combined |= (1 << 6) # four-byte special lower bound
  if i < 0x80 or i > 0x8F:
    combined |= (1 << 7) # four-byte special upper bound
  utf_8_file.write("%d," % combined)

for i in range(128, 256):
  lane = (1 << 2) # invalid lead
  if i >= 0xC2 and i <= 0xDF:
    lane = (1 << 3) # normal trail
  elif i == 0xE0:
    lane = (1 << 4) # three-byte special lower bound
  elif i >= 0xE1 and i <= 0xEC:
    lane = (1 << 3) # normal trail
  elif i == 0xED:
    lane = (1 << 5) # three-byte special upper bound
  elif i >= 0xEE and i <= 0xEF:
    lane = (1 << 3) # normal trail
  elif i == 0xF0:
    lane = (1 << 6) # four-byte special lower bound
  elif i >= 0xF1 and i <= 0xF3:
    lane = (1 << 3) # normal trail
  elif i == 0xF4:
    lane = (1 << 7) # four-byte special upper bound
  utf_8_file.write("%d," % lane)

utf_8_file.write("""
    ],
};

""")

utf_8_file.write(utf_8_rs_end)
utf_8_file.close()

# Unit tests

TEST_HEADER = '''Generated from WHATWG indexes.json; see LICENSE-WHATWG.

This is a generated file. Please do not edit.
Instead, please regenerate using generate-encoding-data.py
'''

index = indexes["jis0208"]

jis0208_in_file = open("src/test_data/jis0208_in.txt", "w")
jis0208_in_file.write(TEST_HEADER)
for pointer in range(0, 94 * 94):
  (lead, trail) = divmod(pointer, 94)
  lead += 0xA1
  trail += 0xA1
  jis0208_in_file.write("%s%s\n" % (chr(lead), chr(trail)))
jis0208_in_file.close()

jis0208_in_ref_file = open("src/test_data/jis0208_in_ref.txt", "w")
jis0208_in_ref_file.write(TEST_HEADER)
for pointer in range(0, 94 * 94):
  code_point = index[pointer]
  if code_point:
    jis0208_in_ref_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
  else:
    jis0208_in_ref_file.write(u"\uFFFD\n".encode("utf-8"))
jis0208_in_ref_file.close()

jis0208_out_file = open("src/test_data/jis0208_out.txt", "w")
jis0208_out_ref_file = open("src/test_data/jis0208_out_ref.txt", "w")
jis0208_out_file.write(TEST_HEADER)
jis0208_out_ref_file.write(TEST_HEADER)
for pointer in range(0, 94 * 94):
  code_point = index[pointer]
  if code_point:
    revised_pointer = pointer
    if revised_pointer == 8644 or (revised_pointer >= 1207 and revised_pointer < 1220):
      revised_pointer = index.index(code_point)
    (lead, trail) = divmod(revised_pointer, 94)
    lead += 0xA1
    trail += 0xA1
    jis0208_out_ref_file.write("%s%s\n" % (chr(lead), chr(trail)))
    jis0208_out_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
jis0208_out_file.close()
jis0208_out_ref_file.close()

shift_jis_in_file = open("src/test_data/shift_jis_in.txt", "w")
shift_jis_in_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  (lead, trail) = divmod(pointer, 188)
  lead += 0x81 if lead < 0x1F else 0xC1
  trail += 0x40 if trail < 0x3F else 0x41
  shift_jis_in_file.write("%s%s\n" % (chr(lead), chr(trail)))
shift_jis_in_file.close()

shift_jis_in_ref_file = open("src/test_data/shift_jis_in_ref.txt", "w")
shift_jis_in_ref_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  code_point = 0xE000 - 8836 + pointer if pointer >= 8836 and pointer <= 10715 else index[pointer]
  if code_point:
    shift_jis_in_ref_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
  else:
    trail = pointer % 188
    trail += 0x40 if trail < 0x3F else 0x41
    if trail < 0x80:
      shift_jis_in_ref_file.write((u"\uFFFD%s\n" % unichr(trail)).encode("utf-8"))
    else:
      shift_jis_in_ref_file.write(u"\uFFFD\n".encode("utf-8"))
shift_jis_in_ref_file.close()

shift_jis_out_file = open("src/test_data/shift_jis_out.txt", "w")
shift_jis_out_ref_file = open("src/test_data/shift_jis_out_ref.txt", "w")
shift_jis_out_file.write(TEST_HEADER)
shift_jis_out_ref_file.write(TEST_HEADER)
for pointer in range(0, 8272):
  code_point = index[pointer]
  if code_point:
    revised_pointer = pointer
    if revised_pointer >= 1207 and revised_pointer < 1220:
      revised_pointer = index.index(code_point)
    (lead, trail) = divmod(revised_pointer, 188)
    lead += 0x81 if lead < 0x1F else 0xC1
    trail += 0x40 if trail < 0x3F else 0x41
    shift_jis_out_ref_file.write("%s%s\n" % (chr(lead), chr(trail)))
    shift_jis_out_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
for pointer in range(8836, len(index)):
  code_point = index[pointer]
  if code_point:
    revised_pointer = index.index(code_point)
    if revised_pointer >= 8272 and revised_pointer < 8836:
      revised_pointer = pointer
    (lead, trail) = divmod(revised_pointer, 188)
    lead += 0x81 if lead < 0x1F else 0xC1
    trail += 0x40 if trail < 0x3F else 0x41
    shift_jis_out_ref_file.write("%s%s\n" % (chr(lead), chr(trail)))
    shift_jis_out_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
shift_jis_out_file.close()
shift_jis_out_ref_file.close()

iso_2022_jp_in_file = open("src/test_data/iso_2022_jp_in.txt", "w")
iso_2022_jp_in_file.write(TEST_HEADER)
for pointer in range(0, 94 * 94):
  (lead, trail) = divmod(pointer, 94)
  lead += 0x21
  trail += 0x21
  iso_2022_jp_in_file.write("\x1B$B%s%s\x1B(B\n" % (chr(lead), chr(trail)))
iso_2022_jp_in_file.close()

iso_2022_jp_in_ref_file = open("src/test_data/iso_2022_jp_in_ref.txt", "w")
iso_2022_jp_in_ref_file.write(TEST_HEADER)
for pointer in range(0, 94 * 94):
  code_point = index[pointer]
  if code_point:
    iso_2022_jp_in_ref_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
  else:
    iso_2022_jp_in_ref_file.write(u"\uFFFD\n".encode("utf-8"))
iso_2022_jp_in_ref_file.close()

iso_2022_jp_out_file = open("src/test_data/iso_2022_jp_out.txt", "w")
iso_2022_jp_out_ref_file = open("src/test_data/iso_2022_jp_out_ref.txt", "w")
iso_2022_jp_out_file.write(TEST_HEADER)
iso_2022_jp_out_ref_file.write(TEST_HEADER)
for pointer in range(0, 94 * 94):
  code_point = index[pointer]
  if code_point:
    revised_pointer = pointer
    if revised_pointer == 8644 or (revised_pointer >= 1207 and revised_pointer < 1220):
      revised_pointer = index.index(code_point)
    (lead, trail) = divmod(revised_pointer, 94)
    lead += 0x21
    trail += 0x21
    iso_2022_jp_out_ref_file.write("\x1B$B%s%s\x1B(B\n" % (chr(lead), chr(trail)))
    iso_2022_jp_out_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
for i in xrange(len(half_width_index)):
  code_point = i + 0xFF61
  normalized_code_point = half_width_index[i]
  pointer = index.index(normalized_code_point)
  (lead, trail) = divmod(pointer, 94)
  lead += 0x21
  trail += 0x21
  iso_2022_jp_out_ref_file.write("\x1B$B%s%s\x1B(B\n" % (chr(lead), chr(trail)))
  iso_2022_jp_out_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
iso_2022_jp_out_file.close()
iso_2022_jp_out_ref_file.close()

index = indexes["euc-kr"]

euc_kr_in_file = open("src/test_data/euc_kr_in.txt", "w")
euc_kr_in_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  (lead, trail) = divmod(pointer, 190)
  lead += 0x81
  trail += 0x41
  euc_kr_in_file.write("%s%s\n" % (chr(lead), chr(trail)))
euc_kr_in_file.close()

euc_kr_in_ref_file = open("src/test_data/euc_kr_in_ref.txt", "w")
euc_kr_in_ref_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  code_point = index[pointer]
  if code_point:
    euc_kr_in_ref_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
  else:
    trail = pointer % 190
    trail += 0x41
    if trail < 0x80:
      euc_kr_in_ref_file.write((u"\uFFFD%s\n" % unichr(trail)).encode("utf-8"))
    else:
      euc_kr_in_ref_file.write(u"\uFFFD\n".encode("utf-8"))
euc_kr_in_ref_file.close()

euc_kr_out_file = open("src/test_data/euc_kr_out.txt", "w")
euc_kr_out_ref_file = open("src/test_data/euc_kr_out_ref.txt", "w")
euc_kr_out_file.write(TEST_HEADER)
euc_kr_out_ref_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  code_point = index[pointer]
  if code_point:
    (lead, trail) = divmod(pointer, 190)
    lead += 0x81
    trail += 0x41
    euc_kr_out_ref_file.write("%s%s\n" % (chr(lead), chr(trail)))
    euc_kr_out_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
euc_kr_out_file.close()
euc_kr_out_ref_file.close()

index = indexes["gb18030"]

gb18030_in_file = open("src/test_data/gb18030_in.txt", "w")
gb18030_in_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  (lead, trail) = divmod(pointer, 190)
  lead += 0x81
  trail += 0x40 if trail < 0x3F else 0x41
  gb18030_in_file.write("%s%s\n" % (chr(lead), chr(trail)))
gb18030_in_file.close()

gb18030_in_ref_file = open("src/test_data/gb18030_in_ref.txt", "w")
gb18030_in_ref_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  code_point = index[pointer]
  if code_point:
    gb18030_in_ref_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
  else:
    trail = pointer % 190
    trail += 0x40 if trail < 0x3F else 0x41
    if trail < 0x80:
      gb18030_in_ref_file.write((u"\uFFFD%s\n" % unichr(trail)).encode("utf-8"))
    else:
      gb18030_in_ref_file.write(u"\uFFFD\n".encode("utf-8"))
gb18030_in_ref_file.close()

gb18030_out_file = open("src/test_data/gb18030_out.txt", "w")
gb18030_out_ref_file = open("src/test_data/gb18030_out_ref.txt", "w")
gb18030_out_file.write(TEST_HEADER)
gb18030_out_ref_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  if pointer == 6555:
    continue
  code_point = index[pointer]
  if code_point:
    (lead, trail) = divmod(pointer, 190)
    lead += 0x81
    trail += 0x40 if trail < 0x3F else 0x41
    gb18030_out_ref_file.write("%s%s\n" % (chr(lead), chr(trail)))
    gb18030_out_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
gb18030_out_file.close()
gb18030_out_ref_file.close()

index = indexes["big5"]

big5_in_file = open("src/test_data/big5_in.txt", "w")
big5_in_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  (lead, trail) = divmod(pointer, 157)
  lead += 0x81
  trail += 0x40 if trail < 0x3F else 0x62
  big5_in_file.write("%s%s\n" % (chr(lead), chr(trail)))
big5_in_file.close()

big5_two_characters = {
  1133: u"\u00CA\u0304",
  1135: u"\u00CA\u030C",
  1164: u"\u00EA\u0304",
  1166: u"\u00EA\u030C",
}

big5_in_ref_file = open("src/test_data/big5_in_ref.txt", "w")
big5_in_ref_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  if pointer in big5_two_characters.keys():
    big5_in_ref_file.write((u"%s\n" % big5_two_characters[pointer]).encode("utf-8"))
    continue
  code_point = index[pointer]
  if code_point:
    big5_in_ref_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
  else:
    trail = pointer % 157
    trail += 0x40 if trail < 0x3F else 0x62
    if trail < 0x80:
      big5_in_ref_file.write((u"\uFFFD%s\n" % unichr(trail)).encode("utf-8"))
    else:
      big5_in_ref_file.write(u"\uFFFD\n".encode("utf-8"))
big5_in_ref_file.close()

prefer_last = [
  0x2550,
  0x255E,
  0x2561,
  0x256A,
  0x5341,
  0x5345,
]

pointer_for_prefer_last = []

for code_point in prefer_last:
  # Python lists don't have .rindex() :-(
  for i in xrange(len(index) - 1, -1, -1):
    candidate = index[i]
    if candidate == code_point:
       pointer_for_prefer_last.append(i)
       break

big5_out_file = open("src/test_data/big5_out.txt", "w")
big5_out_ref_file = open("src/test_data/big5_out_ref.txt", "w")
big5_out_file.write(TEST_HEADER)
big5_out_ref_file.write(TEST_HEADER)
for pointer in range(((0xA1 - 0x81) * 157), len(index)):
  code_point = index[pointer]
  if code_point:
    if code_point in prefer_last:
      if pointer != pointer_for_prefer_last[prefer_last.index(code_point)]:
        continue
    else:
      if pointer != index.index(code_point):
        continue
    (lead, trail) = divmod(pointer, 157)
    lead += 0x81
    trail += 0x40 if trail < 0x3F else 0x62
    big5_out_ref_file.write("%s%s\n" % (chr(lead), chr(trail)))
    big5_out_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
big5_out_file.close()
big5_out_ref_file.close()

index = indexes["jis0212"]

jis0212_in_file = open("src/test_data/jis0212_in.txt", "w")
jis0212_in_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  (lead, trail) = divmod(pointer, 94)
  lead += 0xA1
  trail += 0xA1
  jis0212_in_file.write("\x8F%s%s\n" % (chr(lead), chr(trail)))
jis0212_in_file.close()

jis0212_in_ref_file = open("src/test_data/jis0212_in_ref.txt", "w")
jis0212_in_ref_file.write(TEST_HEADER)
for pointer in range(0, len(index)):
  code_point = index[pointer]
  if code_point:
    jis0212_in_ref_file.write((u"%s\n" % unichr(code_point)).encode("utf-8"))
  else:
    jis0212_in_ref_file.write(u"\uFFFD\n".encode("utf-8"))
jis0212_in_ref_file.close()

(codepage_begin, codepage_end) = read_non_generated("../codepage/src/lib.rs")

codepage_file = open("../codepage/src/lib.rs", "w")

codepage_file.write(codepage_begin)
codepage_file.write("""
// Instead, please regenerate using generate-encoding-data.py

/// Supported code page numbers in estimated order of usage frequency
static CODE_PAGES: [u16; %d] = [
""" % len(code_pages))

for code_page in code_pages:
  codepage_file.write("    %d,\n" % code_page)

codepage_file.write("""];

/// Encodings corresponding to the code page numbers in the same order
static ENCODINGS: [&'static Encoding; %d] = [
""" % len(code_pages))

for code_page in code_pages:
  name = encodings_by_code_page[code_page]
  codepage_file.write("    &%s_INIT,\n" % to_constant_name(name))

codepage_file.write("""];

""")

codepage_file.write(codepage_end)
codepage_file.close()

(codepage_test_begin, codepage_test_end) = read_non_generated("../codepage/src/tests.rs")

codepage_test_file = open("../codepage/src/tests.rs", "w")

codepage_test_file.write(codepage_test_begin)
codepage_test_file.write("""
// Instead, please regenerate using generate-encoding-data.py

#[test]
fn test_to_encoding() {
    assert_eq!(to_encoding(0), None);

""")

for code_page in code_pages:
  codepage_test_file.write("    assert_eq!(to_encoding(%d), Some(%s));\n" % (code_page, to_constant_name(encodings_by_code_page[code_page])))  

codepage_test_file.write("""}

#[test]
fn test_from_encoding() {
""")

for name in preferred:
  if code_pages_by_encoding.has_key(name):
    codepage_test_file.write("    assert_eq!(from_encoding(%s), Some(%d));\n" % (to_constant_name(name), code_pages_by_encoding[name]))
  else:
    codepage_test_file.write("    assert_eq!(from_encoding(%s), None);\n" % to_constant_name(name))

codepage_test_file.write("""}
""")

codepage_test_file.write(codepage_test_end)
codepage_test_file.close()

subprocess.call(["cargo", "fmt"])
