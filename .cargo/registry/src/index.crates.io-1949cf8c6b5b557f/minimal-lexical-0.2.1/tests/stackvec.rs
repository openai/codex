use minimal_lexical::bigint;
#[cfg(feature = "alloc")]
pub use minimal_lexical::heapvec::HeapVec as VecType;
#[cfg(not(feature = "alloc"))]
pub use minimal_lexical::stackvec::StackVec as VecType;

pub fn vec_from_u32(x: &[u32]) -> VecType {
    let mut vec = VecType::new();
    #[cfg(not(all(target_pointer_width = "64", not(target_arch = "sparc"))))]
    {
        for &xi in x {
            vec.try_push(xi as bigint::Limb).unwrap();
        }
    }

    #[cfg(all(target_pointer_width = "64", not(target_arch = "sparc")))]
    {
        for xi in x.chunks(2) {
            match xi.len() {
                1 => vec.try_push(xi[0] as bigint::Limb).unwrap(),
                2 => {
                    let xi0 = xi[0] as bigint::Limb;
                    let xi1 = xi[1] as bigint::Limb;
                    vec.try_push((xi1 << 32) | xi0).unwrap()
                },
                _ => unreachable!(),
            }
        }
    }

    vec
}
