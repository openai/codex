use termtree::Tree;

use std::path::Path;
use std::{env, fs, io};

fn label<P: AsRef<Path>>(p: P) -> String {
    p.as_ref().file_name().unwrap().to_str().unwrap().to_owned()
}

fn tree<P: AsRef<Path>>(p: P) -> io::Result<Tree<String>> {
    let result = fs::read_dir(&p)?.filter_map(|e| e.ok()).fold(
        Tree::new(label(p.as_ref().canonicalize()?)),
        |mut root, entry| {
            let dir = entry.metadata().unwrap();
            if dir.is_dir() {
                root.push(tree(entry.path()).unwrap());
            } else {
                root.push(Tree::new(label(entry.path())));
            }
            root
        },
    );
    Ok(result)
}

fn main() {
    let dir = env::args().nth(1).unwrap_or_else(|| String::from("."));
    match tree(dir) {
        Ok(tree) => println!("{}", tree),
        Err(err) => println!("error: {}", err),
    }
}
