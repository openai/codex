/*
 * // Copyright (c) Radzivon Bartoshyk 3/2025. All rights reserved.
 * //
 * // Redistribution and use in source and binary forms, with or without modification,
 * // are permitted provided that the following conditions are met:
 * //
 * // 1.  Redistributions of source code must retain the above copyright notice, this
 * // list of conditions and the following disclaimer.
 * //
 * // 2.  Redistributions in binary form must reproduce the above copyright notice,
 * // this list of conditions and the following disclaimer in the documentation
 * // and/or other materials provided with the distribution.
 * //
 * // 3.  Neither the name of the copyright holder nor the names of its
 * // contributors may be used to endorse or promote products derived from
 * // this software without specific prior written permission.
 * //
 * // THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * // AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * // IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * // DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 * // FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * // DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * // SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 * // CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 * // OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * // OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
use crate::CmsError;
use crate::writer::write_u16_be;
use std::time::{SystemTime, UNIX_EPOCH};

#[repr(C)]
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Default)]
pub struct ColorDateTime {
    pub year: u16,
    pub month: u16,
    pub day_of_the_month: u16,
    pub hours: u16,
    pub minutes: u16,
    pub seconds: u16,
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 => 31,
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => unreachable!("Unknown month"),
    }
}

impl ColorDateTime {
    /// Parses slice for date time
    pub fn new_from_slice(slice: &[u8]) -> Result<ColorDateTime, CmsError> {
        if slice.len() != 12 {
            return Err(CmsError::InvalidProfile);
        }
        let year = u16::from_be_bytes([slice[0], slice[1]]);
        let month = u16::from_be_bytes([slice[2], slice[3]]);
        let day_of_the_month = u16::from_be_bytes([slice[4], slice[5]]);
        let hours = u16::from_be_bytes([slice[6], slice[7]]);
        let minutes = u16::from_be_bytes([slice[8], slice[9]]);
        let seconds = u16::from_be_bytes([slice[10], slice[11]]);
        Ok(ColorDateTime {
            year,
            month,
            day_of_the_month,
            hours,
            minutes,
            seconds,
        })
    }

    /// Creates a new `ColorDateTime` from the current system time (UTC)
    pub fn now() -> Self {
        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut days = (now.as_secs() / 86_400) as i64;
        let secs_of_day = (now.as_secs() % 86_400) as i64;

        let mut year = 1970;
        loop {
            let year_days = if is_leap(year) { 366 } else { 365 };
            if days >= year_days {
                days -= year_days;
                year += 1;
            } else {
                break;
            }
        }

        let mut month = 1;
        loop {
            let mdays = days_in_month(year, month);
            if days >= mdays as i64 {
                days -= mdays as i64;
                month += 1;
            } else {
                break;
            }
        }
        let day = days + 1; // days from zero based to 1 base

        let hour = secs_of_day / 3600;
        let min = (secs_of_day % 3600) / 60;
        let sec = secs_of_day % 60;
        Self {
            year: year as u16,
            month: month as u16,
            day_of_the_month: day as u16,
            hours: hour as u16,
            minutes: min as u16,
            seconds: sec as u16,
        }
    }

    #[inline]
    pub(crate) fn encode(&self, into: &mut Vec<u8>) {
        let year = self.year;
        let month = self.month;
        let day_of_the_month = self.day_of_the_month;
        let hours = self.hours;
        let minutes = self.minutes;
        let seconds = self.seconds;
        write_u16_be(into, year);
        write_u16_be(into, month);
        write_u16_be(into, day_of_the_month);
        write_u16_be(into, hours);
        write_u16_be(into, minutes);
        write_u16_be(into, seconds);
    }
}
