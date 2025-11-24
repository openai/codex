// musl as a whole is licensed under the following standard MIT license:
//
// ----------------------------------------------------------------------
// Copyright © 2005-2020 Rich Felker, et al.
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
// TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
// SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
// ----------------------------------------------------------------------
//
// Authors/contributors include:
//
// A. Wilcox
// Ada Worcester
// Alex Dowad
// Alex Suykov
// Alexander Monakov
// Andre McCurdy
// Andrew Kelley
// Anthony G. Basile
// Aric Belsito
// Arvid Picciani
// Bartosz Brachaczek
// Benjamin Peterson
// Bobby Bingham
// Boris Brezillon
// Brent Cook
// Chris Spiegel
// Clément Vasseur
// Daniel Micay
// Daniel Sabogal
// Daurnimator
// David Carlier
// David Edelsohn
// Denys Vlasenko
// Dmitry Ivanov
// Dmitry V. Levin
// Drew DeVault
// Emil Renner Berthing
// Fangrui Song
// Felix Fietkau
// Felix Janda
// Gianluca Anzolin
// Hauke Mehrtens
// He X
// Hiltjo Posthuma
// Isaac Dunham
// Jaydeep Patil
// Jens Gustedt
// Jeremy Huntwork
// Jo-Philipp Wich
// Joakim Sindholt
// John Spencer
// Julien Ramseier
// Justin Cormack
// Kaarle Ritvanen
// Khem Raj
// Kylie McClain
// Leah Neukirchen
// Luca Barbato
// Luka Perkov
// M Farkas-Dyck (Strake)
// Mahesh Bodapati
// Markus Wichmann
// Masanori Ogino
// Michael Clark
// Michael Forney
// Mikhail Kremnyov
// Natanael Copa
// Nicholas J. Kain
// orc
// Pascal Cuoq
// Patrick Oppenlander
// Petr Hosek
// Petr Skocik
// Pierre Carrier
// Reini Urban
// Rich Felker
// Richard Pennington
// Ryan Fairfax
// Samuel Holland
// Segev Finer
// Shiz
// sin
// Solar Designer
// Stefan Kristiansson
// Stefan O'Rear
// Szabolcs Nagy
// Timo Teräs
// Trutz Behn
// Valentin Ochs
// Will Dietz
// William Haddon
// William Pitcock
//
// Portions of this software are derived from third-party works licensed
// under terms compatible with the above MIT license:
//
// The TRE regular expression implementation (src/regex/reg* and
// src/regex/tre*) is Copyright © 2001-2008 Ville Laurikari and licensed
// under a 2-clause BSD license (license text in the source files). The
// included version has been heavily modified by Rich Felker in 2012, in
// the interests of size, simplicity, and namespace cleanliness.
//
// Much of the math library code (src/math/* and src/complex/*) is
// Copyright © 1993,2004 Sun Microsystems or
// Copyright © 2003-2011 David Schultz or
// Copyright © 2003-2009 Steven G. Kargl or
// Copyright © 2003-2009 Bruce D. Evans or
// Copyright © 2008 Stephen L. Moshier or
// Copyright © 2017-2018 Arm Limited
// and labelled as such in comments in the individual source files. All
// have been licensed under extremely permissive terms.
//
// The ARM memcpy code (src/string/arm/memcpy.S) is Copyright © 2008
// The Android Open Source Project and is licensed under a two-clause BSD
// license. It was taken from Bionic libc, used on Android.
//
// The AArch64 memcpy and memset code (src/string/aarch64/*) are
// Copyright © 1999-2019, Arm Limited.
//
// The implementation of DES for crypt (src/crypt/crypt_des.c) is
// Copyright © 1994 David Burren. It is licensed under a BSD license.
//
// The implementation of blowfish crypt (src/crypt/crypt_blowfish.c) was
// originally written by Solar Designer and placed into the public
// domain. The code also comes with a fallback permissive license for use
// in jurisdictions that may not recognize the public domain.
//
// The smoothsort implementation (src/stdlib/qsort.c) is Copyright © 2011
// Valentin Ochs and is licensed under an MIT-style license.
//
// The x86_64 port was written by Nicholas J. Kain and is licensed under
// the standard MIT terms.
//
// The mips and microblaze ports were originally written by Richard
// Pennington for use in the ellcc project. The original code was adapted
// by Rich Felker for build system and code conventions during upstream
// integration. It is licensed under the standard MIT terms.
//
// The mips64 port was contributed by Imagination Technologies and is
// licensed under the standard MIT terms.
//
// The powerpc port was also originally written by Richard Pennington,
// and later supplemented and integrated by John Spencer. It is licensed
// under the standard MIT terms.
//
// All other files which have no copyright comments are original works
// produced specifically for use as part of this library, written either
// by Rich Felker, the main author of the library, or by one or more
// contibutors listed above. Details on authorship of individual files
// can be found in the git version control history of the project. The
// omission of copyright and license comments in each file is in the
// interest of source tree size.
//
// In addition, permission is hereby granted for all public header files
// (include/* and arch/*/bits/*) and crt files intended to be linked into
// applications (crt/*, ldso/dlstart.c, and arch/*/crt_arch.h) to omit
// the copyright notice and permission notice otherwise required by the
// license, and to use these files without any requirement of
// attribution. These files include substantial contributions from:
//
// Bobby Bingham
// John Spencer
// Nicholas J. Kain
// Rich Felker
// Richard Pennington
// Stefan Kristiansson
// Szabolcs Nagy
//
// all of whom have explicitly granted such permission.
//
// This file previously contained text expressing a belief that most of
// the files covered by the above exception were sufficiently trivial not
// to be subject to copyright, resulting in confusion over whether it
// negated the permissions granted in the license. In the spirit of
// permissive licensing, and of not having licensing issues being an
// obstacle to adoption, that text has been removed.


use std::fmt;

/// A date/time type which exists primarily to convert `SystemTime` timestamps into an ISO 8601
/// formatted string.
///
/// Yes, this exists. Before you have a heart attack, understand that the meat of this is musl's
/// [`__secs_to_tm`][1] converted to Rust via [c2rust][2] and then cleaned up by hand as part of
/// the [kudu-rs project][3], [released under MIT][4].
///
/// [1] http://git.musl-libc.org/cgit/musl/tree/src/time/__secs_to_tm.c
/// [2] https://c2rust.com/
/// [3] https://github.com/danburkert/kudu-rs/blob/c9660067e5f4c1a54143f169b5eeb49446f82e54/src/timestamp.rs#L5-L18
/// [4] https://github.com/tokio-rs/tracing/issues/1644#issuecomment-963888244
///
/// All existing `strftime`-like APIs I found were unable to handle the full range of timestamps representable
/// by `SystemTime`, including `strftime` itself, since tm.tm_year is an int.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DateTime {
    year: i64,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    nanos: u32,
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.year > 9999 {
            write!(f, "+{}", self.year)?;
        } else if self.year < 0 {
            write!(f, "{:05}", self.year)?;
        } else {
            write!(f, "{:04}", self.year)?;
        }

        write!(
            f,
            "-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
            self.nanos / 1_000
        )
    }
}

impl From<std::time::SystemTime> for DateTime {
    fn from(timestamp: std::time::SystemTime) -> DateTime {
        let (t, nanos) = match timestamp.duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => {
                debug_assert!(duration.as_secs() <= i64::MAX as u64);
                (duration.as_secs() as i64, duration.subsec_nanos())
            }
            Err(error) => {
                let duration = error.duration();
                debug_assert!(duration.as_secs() <= i64::MAX as u64);
                let (secs, nanos) = (duration.as_secs() as i64, duration.subsec_nanos());
                if nanos == 0 {
                    (-secs, 0)
                } else {
                    (-secs - 1, 1_000_000_000 - nanos)
                }
            }
        };

        // 2000-03-01 (mod 400 year, immediately after feb29
        const LEAPOCH: i64 = 946_684_800 + 86400 * (31 + 29);
        const DAYS_PER_400Y: i32 = 365 * 400 + 97;
        const DAYS_PER_100Y: i32 = 365 * 100 + 24;
        const DAYS_PER_4Y: i32 = 365 * 4 + 1;
        static DAYS_IN_MONTH: [i8; 12] = [31, 30, 31, 30, 31, 31, 30, 31, 30, 31, 31, 29];

        // Note(dcb): this bit is rearranged slightly to avoid integer overflow.
        let mut days: i64 = (t / 86_400) - (LEAPOCH / 86_400);
        let mut remsecs: i32 = (t % 86_400) as i32;
        if remsecs < 0i32 {
            remsecs += 86_400;
            days -= 1
        }

        let mut qc_cycles: i32 = (days / i64::from(DAYS_PER_400Y)) as i32;
        let mut remdays: i32 = (days % i64::from(DAYS_PER_400Y)) as i32;
        if remdays < 0 {
            remdays += DAYS_PER_400Y;
            qc_cycles -= 1;
        }

        let mut c_cycles: i32 = remdays / DAYS_PER_100Y;
        if c_cycles == 4 {
            c_cycles -= 1;
        }
        remdays -= c_cycles * DAYS_PER_100Y;

        let mut q_cycles: i32 = remdays / DAYS_PER_4Y;
        if q_cycles == 25 {
            q_cycles -= 1;
        }
        remdays -= q_cycles * DAYS_PER_4Y;

        let mut remyears: i32 = remdays / 365;
        if remyears == 4 {
            remyears -= 1;
        }
        remdays -= remyears * 365;

        let mut years: i64 = i64::from(remyears)
            + 4 * i64::from(q_cycles)
            + 100 * i64::from(c_cycles)
            + 400 * i64::from(qc_cycles);

        let mut months: i32 = 0;
        while i32::from(DAYS_IN_MONTH[months as usize]) <= remdays {
            remdays -= i32::from(DAYS_IN_MONTH[months as usize]);
            months += 1
        }

        if months >= 10 {
            months -= 12;
            years += 1;
        }

        DateTime {
            year: years + 2000,
            month: (months + 3) as u8,
            day: (remdays + 1) as u8,
            hour: (remsecs / 3600) as u8,
            minute: (remsecs / 60 % 60) as u8,
            second: (remsecs % 60) as u8,
            nanos,
        }
    }
}

#[cfg(test)]
mod tests {
    use i32;
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;

    #[test]
    fn test_datetime() {
        let case = |expected: &str, secs: i64, micros: u32| {
            let timestamp = if secs >= 0 {
                UNIX_EPOCH + Duration::new(secs as u64, micros * 1_000)
            } else {
                (UNIX_EPOCH - Duration::new(!secs as u64 + 1, 0)) + Duration::new(0, micros * 1_000)
            };
            assert_eq!(
                expected,
                format!("{}", DateTime::from(timestamp)),
                "secs: {}, micros: {}",
                secs,
                micros
            )
        };

        // Mostly generated with:
        //  - date -jur <secs> +"%Y-%m-%dT%H:%M:%S.000000Z"
        //  - http://unixtimestamp.50x.eu/

        case("1970-01-01T00:00:00.000000Z", 0, 0);

        case("1970-01-01T00:00:00.000001Z", 0, 1);
        case("1970-01-01T00:00:00.500000Z", 0, 500_000);
        case("1970-01-01T00:00:01.000001Z", 1, 1);
        case("1970-01-01T00:01:01.000001Z", 60 + 1, 1);
        case("1970-01-01T01:01:01.000001Z", 60 * 60 + 60 + 1, 1);
        case(
            "1970-01-02T01:01:01.000001Z",
            24 * 60 * 60 + 60 * 60 + 60 + 1,
            1,
        );

        case("1969-12-31T23:59:59.000000Z", -1, 0);
        case("1969-12-31T23:59:59.000001Z", -1, 1);
        case("1969-12-31T23:59:59.500000Z", -1, 500_000);
        case("1969-12-31T23:58:59.000001Z", -60 - 1, 1);
        case("1969-12-31T22:58:59.000001Z", -60 * 60 - 60 - 1, 1);
        case(
            "1969-12-30T22:58:59.000001Z",
            -24 * 60 * 60 - 60 * 60 - 60 - 1,
            1,
        );

        case("2038-01-19T03:14:07.000000Z", i32::MAX as i64, 0);
        case("2038-01-19T03:14:08.000000Z", i32::MAX as i64 + 1, 0);
        case("1901-12-13T20:45:52.000000Z", i32::MIN as i64, 0);
        case("1901-12-13T20:45:51.000000Z", i32::MIN as i64 - 1, 0);

        // Skipping these tests on windows as std::time::SystemTime range is low
        // on Windows compared with that of Unix which can cause the following
        // high date value tests to panic
        #[cfg(not(target_os = "windows"))]
        {
            case("+292277026596-12-04T15:30:07.000000Z", i64::MAX, 0);
            case("+292277026596-12-04T15:30:06.000000Z", i64::MAX - 1, 0);
            case("-292277022657-01-27T08:29:53.000000Z", i64::MIN + 1, 0);
        }

        case("1900-01-01T00:00:00.000000Z", -2208988800, 0);
        case("1899-12-31T23:59:59.000000Z", -2208988801, 0);
        case("0000-01-01T00:00:00.000000Z", -62167219200, 0);
        case("-0001-12-31T23:59:59.000000Z", -62167219201, 0);

        case("1234-05-06T07:08:09.000000Z", -23215049511, 0);
        case("-1234-05-06T07:08:09.000000Z", -101097651111, 0);
        case("2345-06-07T08:09:01.000000Z", 11847456541, 0);
        case("-2345-06-07T08:09:01.000000Z", -136154620259, 0);
    }
}
