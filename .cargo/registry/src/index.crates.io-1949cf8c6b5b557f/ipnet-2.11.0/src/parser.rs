//! A private parser implementation of IPv4 and IPv6 network addresses.
//!
//! The existing `std::net::parser` module cannot be extended because it
//! is private. It is copied and extended here with methods for parsing
//! IP network addresses.

use alloc::{str::FromStr, boxed::Box};
use core::fmt;
#[cfg(not(feature = "std"))]
use core::error::Error;
#[cfg(feature = "std")]
use std::error::Error;
#[cfg(not(feature = "std"))]
use core::net::{Ipv4Addr, Ipv6Addr};
#[cfg(feature = "std")]
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::ipnet::{IpNet, Ipv4Net, Ipv6Net};

pub struct Parser<'a> {
    // parsing as ASCII, so can use byte array
    s: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str) -> Parser<'a> {
        Parser {
            s: s.as_bytes(),
            pos: 0,
        }
    }

    fn is_eof(&self) -> bool {
        self.pos == self.s.len()
    }

    // Commit only if parser returns Some
    fn read_atomically<T, F>(&mut self, cb: F) -> Option<T> where
        F: FnOnce(&mut Parser) -> Option<T>,
    {
        let pos = self.pos;
        let r = cb(self);
        if r.is_none() {
            self.pos = pos;
        }
        r
    }

    // Commit only if parser read till EOF
    fn read_till_eof<T, F>(&mut self, cb: F) -> Option<T> where
        F: FnOnce(&mut Parser) -> Option<T>,
    {
        self.read_atomically(move |p| {
            match cb(p) {
                Some(x) => if p.is_eof() {Some(x)} else {None},
                None => None,
            }
        })
    }

    // Return result of first successful parser
    fn read_or<T>(&mut self, parsers: &mut [Box<dyn FnMut(&mut Parser) -> Option<T> + 'static>])
               -> Option<T> {
        for pf in parsers {
            if let Some(r) = self.read_atomically(|p: &mut Parser| pf(p)) {
                return Some(r);
            }
        }
        None
    }

    // Apply 3 parsers sequentially
    fn read_seq_3<A, B, C, PA, PB, PC>(&mut self,
                                       pa: PA,
                                       pb: PB,
                                       pc: PC)
                                       -> Option<(A, B, C)> where
        PA: FnOnce(&mut Parser) -> Option<A>,
        PB: FnOnce(&mut Parser) -> Option<B>,
        PC: FnOnce(&mut Parser) -> Option<C>,
    {
        self.read_atomically(move |p| {
            let a = pa(p);
            let b = if a.is_some() { pb(p) } else { None };
            let c = if b.is_some() { pc(p) } else { None };
            match (a, b, c) {
                (Some(a), Some(b), Some(c)) => Some((a, b, c)),
                _ => None
            }
        })
    }

    // Read next char
    fn read_char(&mut self) -> Option<char> {
        if self.is_eof() {
            None
        } else {
            let r = self.s[self.pos] as char;
            self.pos += 1;
            Some(r)
        }
    }

    // Return char and advance iff next char is equal to requested
    fn read_given_char(&mut self, c: char) -> Option<char> {
        self.read_atomically(|p| {
            match p.read_char() {
                Some(next) if next == c => Some(next),
                _ => None,
            }
        })
    }

    // Read digit
    fn read_digit(&mut self, radix: u8) -> Option<u8> {
        fn parse_digit(c: char, radix: u8) -> Option<u8> {
            let c = c as u8;
            // assuming radix is either 10 or 16
            if c >= b'0' && c <= b'9' {
                Some(c - b'0')
            } else if radix > 10 && c >= b'a' && c < b'a' + (radix - 10) {
                Some(c - b'a' + 10)
            } else if radix > 10 && c >= b'A' && c < b'A' + (radix - 10) {
                Some(c - b'A' + 10)
            } else {
                None
            }
        }

        self.read_atomically(|p| {
            p.read_char().and_then(|c| parse_digit(c, radix))
        })
    }

    fn read_number_impl(&mut self, radix: u8, max_digits: u32, upto: u32) -> Option<u32> {
        let mut r = 0;
        let mut digit_count = 0;
        loop {
            match self.read_digit(radix) {
                Some(d) => {
                    r = r * (radix as u32) + (d as u32);
                    digit_count += 1;
                    if digit_count > max_digits || r >= upto {
                        return None
                    }
                }
                None => {
                    if digit_count == 0 {
                        return None
                    } else {
                        return Some(r)
                    }
                }
            };
        }
    }

    // Read number, failing if max_digits of number value exceeded
    fn read_number(&mut self, radix: u8, max_digits: u32, upto: u32) -> Option<u32> {
        self.read_atomically(|p| p.read_number_impl(radix, max_digits, upto))
    }

    fn read_ipv4_addr_impl(&mut self) -> Option<Ipv4Addr> {
        let mut bs = [0; 4];
        let mut i = 0;
        while i < 4 {
            if i != 0 && self.read_given_char('.').is_none() {
                return None;
            }

            let octet = self.read_number(10, 3, 0x100).map(|n| n as u8);
            match octet {
                Some(d) => bs[i] = d,
                None => return None,
            };
            i += 1;
        }
        Some(Ipv4Addr::new(bs[0], bs[1], bs[2], bs[3]))
    }

    // Read IPv4 address
    fn read_ipv4_addr(&mut self) -> Option<Ipv4Addr> {
        self.read_atomically(|p| p.read_ipv4_addr_impl())
    }

    fn read_ipv6_addr_impl(&mut self) -> Option<Ipv6Addr> {
        fn ipv6_addr_from_head_tail(head: &[u16], tail: &[u16]) -> Ipv6Addr {
            assert!(head.len() + tail.len() <= 8);
            let mut gs = [0; 8];
            gs[..head.len()].copy_from_slice(head);
            gs[(8 - tail.len()) .. 8].copy_from_slice(tail);
            Ipv6Addr::new(gs[0], gs[1], gs[2], gs[3], gs[4], gs[5], gs[6], gs[7])
        }

        fn read_groups(p: &mut Parser, groups: &mut [u16; 8], limit: usize)
                       -> (usize, bool) {
            let mut i = 0;
            while i < limit {
                if i < limit - 1 {
                    let ipv4 = p.read_atomically(|p| {
                        if i == 0 || p.read_given_char(':').is_some() {
                            p.read_ipv4_addr()
                        } else {
                            None
                        }
                    });
                    if let Some(v4_addr) = ipv4 {
                        let octets = v4_addr.octets();
                        groups[i + 0] = ((octets[0] as u16) << 8) | (octets[1] as u16);
                        groups[i + 1] = ((octets[2] as u16) << 8) | (octets[3] as u16);
                        return (i + 2, true);
                    }
                }

                let group = p.read_atomically(|p| {
                    if i == 0 || p.read_given_char(':').is_some() {
                        p.read_number(16, 4, 0x10000).map(|n| n as u16)
                    } else {
                        None
                    }
                });
                match group {
                    Some(g) => groups[i] = g,
                    None => return (i, false)
                }
                i += 1;
            }
            (i, false)
        }

        let mut head = [0; 8];
        let (head_size, head_ipv4) = read_groups(self, &mut head, 8);

        if head_size == 8 {
            return Some(Ipv6Addr::new(
                head[0], head[1], head[2], head[3],
                head[4], head[5], head[6], head[7]))
        }

        // IPv4 part is not allowed before `::`
        if head_ipv4 {
            return None
        }

        // read `::` if previous code parsed less than 8 groups
        if !self.read_given_char(':').is_some() || !self.read_given_char(':').is_some() {
            return None;
        }

        let mut tail = [0; 8];
        let (tail_size, _) = read_groups(self, &mut tail, 8 - head_size);
        Some(ipv6_addr_from_head_tail(&head[..head_size], &tail[..tail_size]))
    }

    fn read_ipv6_addr(&mut self) -> Option<Ipv6Addr> {
        self.read_atomically(|p| p.read_ipv6_addr_impl())
    }
    
    /* Additions for IpNet below. */

    // Read IPv4 network
    fn read_ipv4_net(&mut self) -> Option<Ipv4Net> {
        let ip_addr = |p: &mut Parser| p.read_ipv4_addr();
        let slash = |p: &mut Parser| p.read_given_char('/');
        let prefix_len = |p: &mut Parser| {
            p.read_number(10, 2, 33).map(|n| n as u8)
        };

        self.read_seq_3(ip_addr, slash, prefix_len).map(|t| {
            let (ip, _, prefix_len): (Ipv4Addr, char, u8) = t;
            Ipv4Net::new(ip, prefix_len).unwrap()
        })
    }

    // Read Ipv6 network
    fn read_ipv6_net(&mut self) -> Option<Ipv6Net> {
        let ip_addr = |p: &mut Parser| p.read_ipv6_addr();
        let slash = |p: &mut Parser| p.read_given_char('/');
        let prefix_len = |p: &mut Parser| {
            p.read_number(10, 3, 129).map(|n| n as u8)
        };

        self.read_seq_3(ip_addr, slash, prefix_len).map(|t| {
            let (ip, _, prefix_len): (Ipv6Addr, char, u8) = t;
            Ipv6Net::new(ip, prefix_len).unwrap()
        })
    }

    fn read_ip_net(&mut self) -> Option<IpNet> {
        let ipv4_net = |p: &mut Parser| p.read_ipv4_net().map(IpNet::V4);
        let ipv6_net = |p: &mut Parser| p.read_ipv6_net().map(IpNet::V6);
        self.read_or(&mut [Box::new(ipv4_net), Box::new(ipv6_net)])
    }

    /* Additions for IpNet above. */
}

/* Additions for IpNet below. */

impl FromStr for IpNet {
    type Err = AddrParseError;
    fn from_str(s: &str) -> Result<IpNet, AddrParseError> {
        match Parser::new(s).read_till_eof(|p| p.read_ip_net()) {
            Some(s) => Ok(s),
            None => Err(AddrParseError(()))
        }
    }
}

impl FromStr for Ipv4Net {
    type Err = AddrParseError;
    fn from_str(s: &str) -> Result<Ipv4Net, AddrParseError> {
        match Parser::new(s).read_till_eof(|p| p.read_ipv4_net()) {
            Some(s) => Ok(s),
            None => Err(AddrParseError(()))
        }
    }
}

impl FromStr for Ipv6Net {
    type Err = AddrParseError;
    fn from_str(s: &str) -> Result<Ipv6Net, AddrParseError> {
        match Parser::new(s).read_till_eof(|p| p.read_ipv6_net()) {
            Some(s) => Ok(s),
            None => Err(AddrParseError(()))
        }
    }
}

/* Additions for IpNet above. */

/// An error which can be returned when parsing an IP network address.
///
/// This error is used as the error type for the [`FromStr`] implementation for
/// [`IpNet`], [`Ipv4Net`], and [`Ipv6Net`].
///
/// [`FromStr`]: https://doc.rust-lang.org/std/str/trait.FromStr.html
/// [`IpNet`]: enum.IpNet.html
/// [`Ipv4Net`]: struct.Ipv4Net.html
/// [`Ipv6Net`]: struct.Ipv6Net.html
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddrParseError(());

impl fmt::Display for AddrParseError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("invalid IP address syntax")
    }
}

impl Error for AddrParseError {}
