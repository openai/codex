//! Test Cases

extern crate alloc;

mod utils;

use crate::utils::is_equal;
use alloc::collections::BTreeMap;
use expect_test::expect;
use serde::{Deserialize, Serialize};
use serde_with::with_prefix;
use std::collections::HashMap;

#[test]
fn test_flatten_with_prefix() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Match {
        #[serde(flatten, with = "prefix_player1")]
        player1: Player,
        #[serde(flatten, with = "prefix_player2")]
        player2: Option<Player>,
        #[serde(flatten, with = "prefix_player3")]
        player3: Option<Player>,
        #[serde(flatten, with = "prefix_tag")]
        tags: HashMap<String, String>,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Player {
        name: String,
        votes: u64,
    }

    with_prefix!(prefix_player1 "player1_");
    with_prefix!(prefix_player2 "player2_");
    with_prefix!(prefix_player3 "player3_");
    with_prefix!(prefix_tag "tag_");

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
              "player1_name": "name1",
              "player1_votes": 1,
              "player2_name": "name2",
              "player2_votes": 2,
              "tag_t": "T"
            }"#]],
    );
}

#[test]
fn test_plain_with_prefix() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Match {
        #[serde(with = "prefix_player1")]
        player1: Player,
        #[serde(with = "prefix_player2")]
        player2: Option<Player>,
        #[serde(with = "prefix_player3")]
        player3: Option<Player>,
        #[serde(with = "prefix_tag")]
        tags: HashMap<String, String>,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Player {
        name: String,
        votes: u64,
    }

    with_prefix!(prefix_player1 "player1_");
    with_prefix!(prefix_player2 "player2_");
    with_prefix!(prefix_player3 "player3_");
    with_prefix!(prefix_tag "tag_");

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
            "player1_name": "name1",
            "player1_votes": 1
          },
          "player2": {
            "player2_name": "name2",
            "player2_votes": 2
          },
          "player3": null,
          "tags": {
            "tag_t": "T"
          }
        }"#]],
    );
}

/// Ensure that `with_prefix` works for unit type enum variants.
#[test]
fn test_enum_unit_variant_with_prefix() {
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
    with_prefix!(foo "foo_");

    let d = Data {
        stuff: "Stuff".to_owned(),
        foo: BTreeMap::from_iter(vec![(Foo::One, 1), (Foo::Two, 2), (Foo::Three, 3)]),
    };

    is_equal(
        d,
        expect![[r#"
        {
          "stuff": "Stuff",
          "foo_One": 1,
          "foo_Two": 2,
          "foo_Three": 3
        }"#]],
    );
}
