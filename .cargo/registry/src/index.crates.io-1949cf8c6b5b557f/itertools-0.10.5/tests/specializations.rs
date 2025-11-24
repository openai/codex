use itertools::Itertools;
use std::fmt::Debug;
use quickcheck::quickcheck;

struct Unspecialized<I>(I);
impl<I> Iterator for Unspecialized<I>
where
    I: Iterator,
{
    type Item = I::Item;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

macro_rules! check_specialized {
    ($src:expr, |$it:pat| $closure:expr) => {
        let $it = $src.clone();
        let v1 = $closure;

        let $it = Unspecialized($src.clone());
        let v2 = $closure;

        assert_eq!(v1, v2);
    }
}

fn test_specializations<IterItem, Iter>(
    it: &Iter,
) where
    IterItem: Eq + Debug + Clone,
    Iter: Iterator<Item = IterItem> + Clone,
{
    check_specialized!(it, |i| i.count());
    check_specialized!(it, |i| i.last());
    check_specialized!(it, |i| i.collect::<Vec<_>>());
    check_specialized!(it, |i| {
        let mut parameters_from_fold = vec![];
        let fold_result = i.fold(vec![], |mut acc, v: IterItem| {
            parameters_from_fold.push((acc.clone(), v.clone()));
            acc.push(v);
            acc
        });
        (parameters_from_fold, fold_result)
    });
    check_specialized!(it, |mut i| {
        let mut parameters_from_all = vec![];
        let first = i.next();
        let all_result = i.all(|x| {
            parameters_from_all.push(x.clone());
            Some(x)==first
        });
        (parameters_from_all, all_result)
    });
    let size = it.clone().count();
    for n in 0..size + 2 {
        check_specialized!(it, |mut i| i.nth(n));
    }
    // size_hint is a bit harder to check
    let mut it_sh = it.clone();
    for n in 0..size + 2 {
        let len = it_sh.clone().count();
        let (min, max) = it_sh.size_hint();
        assert_eq!(size - n.min(size), len);
        assert!(min <= len);
        if let Some(max) = max {
            assert!(len <= max);
        }
        it_sh.next();
    }
}

quickcheck! {
    fn intersperse(v: Vec<u8>) -> () {
        test_specializations(&v.into_iter().intersperse(0));
    }
}

quickcheck! {
    fn put_back_qc(test_vec: Vec<i32>) -> () {
        test_specializations(&itertools::put_back(test_vec.iter()));
        let mut pb = itertools::put_back(test_vec.into_iter());
        pb.put_back(1);
        test_specializations(&pb);
    }
}

quickcheck! {
    fn merge_join_by_qc(i1: Vec<usize>, i2: Vec<usize>) -> () {
        test_specializations(&i1.into_iter().merge_join_by(i2.into_iter(), std::cmp::Ord::cmp));
    }
}

quickcheck! {
    fn map_into(v: Vec<u8>) -> () {
        test_specializations(&v.into_iter().map_into::<u32>());
    }
}

quickcheck! {
    fn map_ok(v: Vec<Result<u8, char>>) -> () {
        test_specializations(&v.into_iter().map_ok(|u| u.checked_add(1)));
    }
}

quickcheck! {
    fn process_results(v: Vec<Result<u8, u8>>) -> () {
        helper(v.iter().copied());
        helper(v.iter().copied().filter(Result::is_ok));

        fn helper(it: impl Iterator<Item = Result<u8, u8>> + Clone) {
            macro_rules! check_results_specialized {
                ($src:expr, |$it:pat| $closure:expr) => {
                    assert_eq!(
                        itertools::process_results($src.clone(), |$it| $closure),
                        itertools::process_results($src.clone(), |i| {
                            let $it = Unspecialized(i);
                            $closure
                        }),
                    )
                }
            }

            check_results_specialized!(it, |i| i.count());
            check_results_specialized!(it, |i| i.last());
            check_results_specialized!(it, |i| i.collect::<Vec<_>>());
            check_results_specialized!(it, |i| {
                let mut parameters_from_fold = vec![];
                let fold_result = i.fold(vec![], |mut acc, v| {
                    parameters_from_fold.push((acc.clone(), v));
                    acc.push(v);
                    acc
                });
                (parameters_from_fold, fold_result)
            });
            check_results_specialized!(it, |mut i| {
                let mut parameters_from_all = vec![];
                let first = i.next();
                let all_result = i.all(|x| {
                    parameters_from_all.push(x);
                    Some(x)==first
                });
                (parameters_from_all, all_result)
            });
            let size = it.clone().count();
            for n in 0..size + 2 {
                check_results_specialized!(it, |mut i| i.nth(n));
            }
        }
    }
}
