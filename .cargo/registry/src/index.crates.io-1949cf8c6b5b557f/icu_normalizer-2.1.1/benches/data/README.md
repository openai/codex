# Generating microbench data

The full versions of these files are located 
[in another part of the repository](https://github.com/unicode-org/icu/tree/main/icu4j/perf-tests/data).

## Sanitizing the file

```shell
sed -i '/^#/d' ${filename}
sed -i '/^$/d' ${filename}
```

## Shuffling the file

```shell
shuf -n 20 ${filename} -o ${filename}
```

## Add back the header (if you plan on submitting the files)

```
# This file is part of ICU4X. For terms of use, please see the file
# called LICENSE at the top level of the ICU4X source tree
# (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).
```
