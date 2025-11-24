/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Write CSV files.

use std::fmt::Debug;
use std::fmt::Display;

use crate::eval::runtime::small_duration::SmallDuration;

/// Writer for CSV files.
pub(crate) struct CsvWriter {
    /// Column count in CSV file.
    column_count: usize,
    /// While writing a row, this is the current column index.
    current_column_index: usize,
    /// Write CSV there.
    buf: String,
}

fn quote_str_for_csv(s: &str) -> String {
    format!("\"{}\"", s.replace('\"', "\"\""))
}

impl CsvWriter {
    pub(crate) fn new<'c>(columns: impl IntoIterator<Item = &'c str>) -> CsvWriter {
        let mut buf = String::new();
        let mut column_count = 0;
        for (i, column) in columns.into_iter().enumerate() {
            if i != 0 {
                buf.push(',');
            }
            buf.push_str(column);
            column_count = i + 1;
        }
        buf.push('\n');
        CsvWriter {
            column_count,
            current_column_index: 0,
            buf,
        }
    }

    pub(crate) fn write_value(&mut self, value: impl CsvValue) {
        assert!(self.current_column_index < self.column_count);
        if self.current_column_index != 0 {
            self.buf.push(',');
        }
        self.buf.push_str(&value.format_for_csv());
        self.current_column_index += 1;
    }

    pub(crate) fn write_display(&mut self, value: impl Display) {
        struct Impl<V: Display>(V);

        impl<V: Display> CsvValue for Impl<V> {
            fn format_for_csv(&self) -> String {
                quote_str_for_csv(&self.0.to_string())
            }
        }

        self.write_value(Impl(value))
    }

    pub(crate) fn write_debug(&mut self, value: impl Debug) {
        struct Impl<V: Debug>(V);

        impl<V: Debug> CsvValue for Impl<V> {
            fn format_for_csv(&self) -> String {
                quote_str_for_csv(&format!("{:?}", &self.0))
            }
        }

        self.write_value(Impl(value))
    }

    pub(crate) fn finish_row(&mut self) {
        assert_eq!(self.current_column_index, self.column_count);
        self.current_column_index = 0;
        self.buf.push('\n');
    }

    pub(crate) fn finish(self) -> String {
        assert!(self.current_column_index == 0);
        self.buf
    }
}

pub(crate) trait CsvValue {
    fn format_for_csv(&self) -> String;
}

impl CsvValue for SmallDuration {
    fn format_for_csv(&self) -> String {
        format!("{:.3}", self.to_duration().as_secs_f64())
    }
}

impl<V: CsvValue + ?Sized> CsvValue for &'_ V {
    fn format_for_csv(&self) -> String {
        (*self).format_for_csv()
    }
}

impl CsvValue for str {
    fn format_for_csv(&self) -> String {
        quote_str_for_csv(self)
    }
}

impl CsvValue for usize {
    fn format_for_csv(&self) -> String {
        self.to_string()
    }
}

impl CsvValue for u64 {
    fn format_for_csv(&self) -> String {
        self.to_string()
    }
}

impl CsvValue for i32 {
    fn format_for_csv(&self) -> String {
        self.to_string()
    }
}

impl CsvValue for u128 {
    fn format_for_csv(&self) -> String {
        self.to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::eval::runtime::profile::csv::quote_str_for_csv;
    use crate::eval::runtime::profile::csv::CsvWriter;
    use crate::eval::runtime::small_duration::SmallDuration;

    #[test]
    fn test_csv_writer() {
        let mut csv = CsvWriter::new(["File", "Count", "Duration"]);
        csv.write_value("a.bzl");
        csv.write_value(10);
        csv.write_value(SmallDuration { nanos: 17_000_000 });
        csv.finish_row();
        csv.write_value("b.bzl");
        csv.write_value(20);
        csv.write_value(SmallDuration { nanos: 19_000_000 });
        csv.finish_row();
        assert_eq!(
            "\
File,Count,Duration
\"a.bzl\",10,0.017
\"b.bzl\",20,0.019
",
            csv.finish()
        )
    }

    #[test]
    fn test_quote_str_for_csv() {
        assert_eq!("\"a\"", quote_str_for_csv("a"));
        assert_eq!("\"a\"\"\"", quote_str_for_csv("a\""));
    }
}
