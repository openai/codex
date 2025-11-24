// Currently this test doesn't actually check the output of the functions.
// It's only here for miri to check for any potential undefined behaviour.
// TODO: check function results

#[test]
fn test_transparent_wrapper() {
  // An external type defined in a different crate.
  #[derive(Debug, Copy, Clone, Default)]
  struct Foreign(u8);

  use bytemuck::TransparentWrapper;

  #[derive(Debug, Copy, Clone)]
  #[repr(transparent)]
  struct Wrapper(Foreign);

  unsafe impl TransparentWrapper<Foreign> for Wrapper {}

  // Traits can be implemented on crate-local wrapper.
  unsafe impl bytemuck::Zeroable for Wrapper {}
  unsafe impl bytemuck::Pod for Wrapper {}

  impl PartialEq<u8> for Foreign {
    fn eq(&self, &other: &u8) -> bool {
      self.0 == other
    }
  }

  impl PartialEq<u8> for Wrapper {
    fn eq(&self, &other: &u8) -> bool {
      self.0 == other
    }
  }

  let _: u8 = bytemuck::cast(Wrapper::wrap(Foreign::default()));
  let _: Foreign = Wrapper::peel(bytemuck::cast(u8::default()));

  let _: &u8 = bytemuck::cast_ref(Wrapper::wrap_ref(&Foreign::default()));
  let _: &Foreign = Wrapper::peel_ref(bytemuck::cast_ref(&u8::default()));

  let _: &mut u8 =
    bytemuck::cast_mut(Wrapper::wrap_mut(&mut Foreign::default()));
  let _: &mut Foreign =
    Wrapper::peel_mut(bytemuck::cast_mut(&mut u8::default()));

  let _: &[u8] =
    bytemuck::cast_slice(Wrapper::wrap_slice(&[Foreign::default()]));
  let _: &[Foreign] =
    Wrapper::peel_slice(bytemuck::cast_slice(&[u8::default()]));

  let _: &mut [u8] =
    bytemuck::cast_slice_mut(Wrapper::wrap_slice_mut(
      &mut [Foreign::default()],
    ));
  let _: &mut [Foreign] =
    Wrapper::peel_slice_mut(bytemuck::cast_slice_mut(&mut [u8::default()]));

  let _: &[u8] = bytemuck::bytes_of(Wrapper::wrap_ref(&Foreign::default()));
  let _: &Foreign = Wrapper::peel_ref(bytemuck::from_bytes(&[u8::default()]));

  let _: &mut [u8] =
    bytemuck::bytes_of_mut(Wrapper::wrap_mut(&mut Foreign::default()));
  let _: &mut Foreign =
    Wrapper::peel_mut(bytemuck::from_bytes_mut(&mut [u8::default()]));

  // not sure if this is the right usage
  let _ =
    bytemuck::pod_align_to::<_, u8>(Wrapper::wrap_slice(&[Foreign::default()]));
  // counterpart?

  // not sure if this is the right usage
  let _ = bytemuck::pod_align_to_mut::<_, u8>(Wrapper::wrap_slice_mut(&mut [
    Foreign::default(),
  ]));
  // counterpart?

  #[cfg(feature = "extern_crate_alloc")]
  {
    use bytemuck::allocation::TransparentWrapperAlloc;
    use std::rc::Rc;

    let a: Vec<Foreign> = vec![Foreign::default(); 2];

    let b: Vec<Wrapper> = Wrapper::wrap_vec(a);
    assert_eq!(b, [0, 0]);

    let c: Vec<Foreign> = Wrapper::peel_vec(b);
    assert_eq!(c, [0, 0]);

    let d: Box<Foreign> = Box::new(Foreign::default());

    let e: Box<Wrapper> = Wrapper::wrap_box(d);
    assert_eq!(&*e, &0);
    let f: Box<Foreign> = Wrapper::peel_box(e);
    assert_eq!(&*f, &0);

    let g: Rc<Foreign> = Rc::new(Foreign::default());

    let h: Rc<Wrapper> = Wrapper::wrap_rc(g);
    assert_eq!(&*h, &0);
    let i: Rc<Foreign> = Wrapper::peel_rc(h);
    assert_eq!(&*i, &0);

    #[cfg(target_has_atomic = "ptr")]
    {
      use std::sync::Arc;

      let j: Arc<Foreign> = Arc::new(Foreign::default());

      let k: Arc<Wrapper> = Wrapper::wrap_arc(j);
      assert_eq!(&*k, &0);
      let l: Arc<Foreign> = Wrapper::peel_arc(k);
      assert_eq!(&*l, &0);
    }
  }
}
