use crate::algorithms::DiffHook;

/// A [`DiffHook`] that combines deletions and insertions to give blocks
/// of maximal length, and replacements when appropriate.
///
/// It will replace [`DiffHook::insert`] and [`DiffHook::delete`] events when
/// possible with [`DiffHook::replace`] events.  Note that even though the
/// text processing in the crate does not use replace events and always resolves
/// then back to delete and insert, it's useful to always use the replacer to
/// ensure a consistent order of inserts and deletes.  This is why for instance
/// the text diffing automatically uses this hook internally.
pub struct Replace<D: DiffHook> {
    d: D,
    del: Option<(usize, usize, usize)>,
    ins: Option<(usize, usize, usize)>,
    eq: Option<(usize, usize, usize)>,
}

impl<D: DiffHook> Replace<D> {
    /// Creates a new replace hook wrapping another hook.
    pub fn new(d: D) -> Self {
        Replace {
            d,
            del: None,
            ins: None,
            eq: None,
        }
    }

    /// Extracts the inner hook.
    pub fn into_inner(self) -> D {
        self.d
    }

    fn flush_eq(&mut self) -> Result<(), D::Error> {
        if let Some((eq_old_index, eq_new_index, eq_len)) = self.eq.take() {
            self.d.equal(eq_old_index, eq_new_index, eq_len)?
        }
        Ok(())
    }

    fn flush_del_ins(&mut self) -> Result<(), D::Error> {
        if let Some((del_old_index, del_old_len, del_new_index)) = self.del.take() {
            if let Some((_, ins_new_index, ins_new_len)) = self.ins.take() {
                self.d
                    .replace(del_old_index, del_old_len, ins_new_index, ins_new_len)?;
            } else {
                self.d.delete(del_old_index, del_old_len, del_new_index)?;
            }
        } else if let Some((ins_old_index, ins_new_index, ins_new_len)) = self.ins.take() {
            self.d.insert(ins_old_index, ins_new_index, ins_new_len)?;
        }
        Ok(())
    }
}

impl<D: DiffHook> AsRef<D> for Replace<D> {
    fn as_ref(&self) -> &D {
        &self.d
    }
}

impl<D: DiffHook> AsMut<D> for Replace<D> {
    fn as_mut(&mut self) -> &mut D {
        &mut self.d
    }
}

impl<D: DiffHook> DiffHook for Replace<D> {
    type Error = D::Error;

    fn equal(&mut self, old_index: usize, new_index: usize, len: usize) -> Result<(), D::Error> {
        self.flush_del_ins()?;

        self.eq = if let Some((eq_old_index, eq_new_index, eq_len)) = self.eq.take() {
            Some((eq_old_index, eq_new_index, eq_len + len))
        } else {
            Some((old_index, new_index, len))
        };

        Ok(())
    }

    fn delete(
        &mut self,
        old_index: usize,
        old_len: usize,
        new_index: usize,
    ) -> Result<(), D::Error> {
        self.flush_eq()?;
        if let Some((del_old_index, del_old_len, del_new_index)) = self.del.take() {
            debug_assert_eq!(old_index, del_old_index + del_old_len);
            self.del = Some((del_old_index, del_old_len + old_len, del_new_index));
        } else {
            self.del = Some((old_index, old_len, new_index));
        }
        Ok(())
    }

    fn insert(
        &mut self,
        old_index: usize,
        new_index: usize,
        new_len: usize,
    ) -> Result<(), D::Error> {
        self.flush_eq()?;
        self.ins = if let Some((ins_old_index, ins_new_index, ins_new_len)) = self.ins.take() {
            debug_assert_eq!(ins_new_index + ins_new_len, new_index);
            Some((ins_old_index, ins_new_index, new_len + ins_new_len))
        } else {
            Some((old_index, new_index, new_len))
        };

        Ok(())
    }

    fn replace(
        &mut self,
        old_index: usize,
        old_len: usize,
        new_index: usize,
        new_len: usize,
    ) -> Result<(), D::Error> {
        self.flush_eq()?;
        self.d.replace(old_index, old_len, new_index, new_len)
    }

    fn finish(&mut self) -> Result<(), D::Error> {
        self.flush_eq()?;
        self.flush_del_ins()?;
        self.d.finish()
    }
}

#[test]
fn test_mayers_replace() {
    use crate::algorithms::{diff_slices, Algorithm};
    let a: &[&str] = &[
        ">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>\n",
        "a\n",
        "b\n",
        "c\n",
        "================================\n",
        "d\n",
        "e\n",
        "f\n",
        "<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<\n",
    ];
    let b: &[&str] = &[
        ">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>\n",
        "x\n",
        "b\n",
        "c\n",
        "================================\n",
        "y\n",
        "e\n",
        "f\n",
        "<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<\n",
    ];

    let mut d = Replace::new(crate::algorithms::Capture::new());
    diff_slices(Algorithm::Myers, &mut d, a, b).unwrap();

    insta::assert_debug_snapshot!(&d.into_inner().ops(), @r###"
    [
        Equal {
            old_index: 0,
            new_index: 0,
            len: 1,
        },
        Replace {
            old_index: 1,
            old_len: 1,
            new_index: 1,
            new_len: 1,
        },
        Equal {
            old_index: 2,
            new_index: 2,
            len: 3,
        },
        Replace {
            old_index: 5,
            old_len: 1,
            new_index: 5,
            new_len: 1,
        },
        Equal {
            old_index: 6,
            new_index: 6,
            len: 3,
        },
    ]
    "###);
}

#[test]
fn test_replace() {
    use crate::algorithms::{diff_slices, Algorithm};

    let a: &[usize] = &[0, 1, 2, 3, 4];
    let b: &[usize] = &[0, 1, 2, 7, 8, 9];

    let mut d = Replace::new(crate::algorithms::Capture::new());
    diff_slices(Algorithm::Myers, &mut d, a, b).unwrap();
    insta::assert_debug_snapshot!(d.into_inner().ops(), @r###"
    [
        Equal {
            old_index: 0,
            new_index: 0,
            len: 3,
        },
        Replace {
            old_index: 3,
            old_len: 2,
            new_index: 3,
            new_len: 3,
        },
    ]
    "###);
}
