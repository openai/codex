use basic_toml::to_string;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
struct User {
    pub name: String,
    pub surname: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
struct Users {
    pub user: Vec<User>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
struct TwoUsers {
    pub user0: User,
    pub user1: User,
}

#[test]
fn no_unnecessary_newlines_array() {
    assert!(!to_string(&Users {
        user: vec![
            User {
                name: "John".to_string(),
                surname: "Doe".to_string(),
            },
            User {
                name: "Jane".to_string(),
                surname: "Dough".to_string(),
            },
        ],
    })
    .unwrap()
    .starts_with('\n'));
}

#[test]
fn no_unnecessary_newlines_table() {
    assert!(!to_string(&TwoUsers {
        user0: User {
            name: "John".to_string(),
            surname: "Doe".to_string(),
        },
        user1: User {
            name: "Jane".to_string(),
            surname: "Dough".to_string(),
        },
    })
    .unwrap()
    .starts_with('\n'));
}
