//! Rust implementation of the `CPU_*` macro API.

#![allow(non_snake_case)]

use super::types::RawCpuSet;
use core::mem::{size_of, size_of_val};

#[inline]
pub(crate) fn CPU_SET(cpu: usize, cpuset: &mut RawCpuSet) {
    let size_in_bits = 8 * size_of_val(&cpuset.bits[0]); // 32, 64 etc
    let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
    cpuset.bits[idx] |= 1 << offset
}

#[inline]
pub(crate) fn CPU_ZERO(cpuset: &mut RawCpuSet) {
    cpuset.bits.fill(0)
}

#[inline]
pub(crate) fn CPU_CLR(cpu: usize, cpuset: &mut RawCpuSet) {
    let size_in_bits = 8 * size_of_val(&cpuset.bits[0]); // 32, 64 etc
    let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
    cpuset.bits[idx] &= !(1 << offset)
}

#[inline]
pub(crate) fn CPU_ISSET(cpu: usize, cpuset: &RawCpuSet) -> bool {
    let size_in_bits = 8 * size_of_val(&cpuset.bits[0]);
    let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
    (cpuset.bits[idx] & (1 << offset)) != 0
}

#[inline]
pub(crate) fn CPU_COUNT_S(size_in_bytes: usize, cpuset: &RawCpuSet) -> u32 {
    let size_of_mask = size_of_val(&cpuset.bits[0]);
    let idx = size_in_bytes / size_of_mask;
    cpuset.bits[..idx]
        .iter()
        .fold(0, |acc, i| acc + i.count_ones())
}

#[inline]
pub(crate) fn CPU_COUNT(cpuset: &RawCpuSet) -> u32 {
    CPU_COUNT_S(size_of::<RawCpuSet>(), cpuset)
}

#[inline]
pub(crate) fn CPU_EQUAL(this: &RawCpuSet, that: &RawCpuSet) -> bool {
    this.bits == that.bits
}
