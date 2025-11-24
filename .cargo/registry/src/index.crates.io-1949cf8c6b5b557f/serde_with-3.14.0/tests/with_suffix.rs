//! Test Cases

extern crate alloc;

mod utils;

use crate::utils::is_equal;
use alloc::collections::BTreeMap;
use expect_test::expect;
use serde::{Deserialize, Serialize};
use serde_with::with_suffix;
use std::collections::HashMap;

#[test]
fn test_flatten_with_suffix() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Match {
        #[serde(flatten, with = "suffix_player1")]
        player1: Player,
        #[serde(flatten, with = "suffix_player2")]
        player2: Option<Player>,
        #[serde(flatten, with = "suffix_player3")]
        player3: Option<Player>,
        #[serde(flatten, with = "suffix_tag")]
        tags: HashMap<String, String>,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Player {
        name: String,
        votes: u64,
    }

    with_suffix!(suffix_player1 "_player1");
    with_suffix!(suffix_player2 "_player2");
    with_suffix!(suffix_player3 "_player3");
    with_suffix!(suffix_tag "_tag");

    let m = Match {
        player1: Player {
            name: "name1".to_owned(),
            votes: 1,
        },
        player2: Some(Player {
            name: "name2".to_owned(),
            votes: 2,
        }),
        player3: None,
        tags: HashMap::from_iter(vec![("t".to_owned(), "T".to_owned())]),
    };

    is_equal(
        m,
        expect![[r#"
            {
              "name_player1": "name1",
              "votes_player1": 1,
              "name_player2": "name2",
              "votes_player2": 2,
              "t_tag": "T"
            }"#]],
    );
}

#[test]
fn test_plain_with_suffix() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Match {
        #[serde(with = "suffix_player1")]
        player1: Player,
        #[serde(with = "suffix_player2")]
        player2: Option<Player>,
        #[serde(with = "suffix_player3")]
        player3: Option<Player>,
        #[serde(with = "suffix_tag")]
        tags: HashMap<String, String>,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Player {
        name: String,
        votes: u64,
    }

    with_suffix!(suffix_player1 "_player1");
    with_suffix!(suffix_player2 "_player2");
    with_suffix!(suffix_player3 "_player3");
    with_suffix!(suffix_tag "_tag");

    let m = Match {
        player1: Player {
            name: "name1".to_owned(),
            votes: 1,
        },
        player2: Some(Player {
            name: "name2".to_owned(),
            votes: 2,
        }),
        player3: None,
        tags: HashMap::from_iter(vec![("t".to_owned(), "T".to_owned())]),
    };

    is_equal(
        m,
        expect![[r#"
        {
          "player1": {
            "name_player1": "name1",
            "votes_player1": 1
          },
          "player2": {
            "name_player2": "name2",
            "votes_player2": 2
          },
          "player3": null,
          "tags": {
            "t_tag": "T"
          }
        }"#]],
    );
}

/// Ensure that `with_suffix` works for unit type enum variants.
#[test]
fn test_enum_unit_variant_with_suffix() {
    #[derive(Hash, PartialEq, Eq, Debug, Serialize, Deserialize, Ord, PartialOrd)]
    enum Foo {
        One,
        Two,
        Three,
    }

    #[derive(Hash, PartialEq, Eq, Debug, Serialize, Deserialize, Ord, PartialOrd)]
    struct Data {
        stuff: String,

        #[serde(flatten, with = "foo")]
        foo: BTreeMap<Foo, i32>,
    }
    with_suffix!(foo "_foo");

    let d = Data {
        stuff: "Stuff".to_owned(),
        foo: BTreeMap::from_iter(vec![(Foo::One, 1), (Foo::Two, 2), (Foo::Three, 3)]),
    };

    is_equal(
        d,
        expect![[r#"
        {
          "stuff": "Stuff",
          "One_foo": 1,
          "Two_foo": 2,
          "Three_foo": 3
        }"#]],
    );
}
