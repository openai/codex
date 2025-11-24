use std::{env, io::Write, path::Path};

use {bstr::ByteVec, ignore::WalkBuilder, walkdir::WalkDir};

fn main() {
    let mut path = env::args().nth(1).unwrap();
    let mut parallel = false;
    let mut simple = false;
    let (tx, rx) = crossbeam_channel::bounded::<DirEntry>(100);
    if path == "parallel" {
        path = env::args().nth(2).unwrap();
        parallel = true;
    } else if path == "walkdir" {
        path = env::args().nth(2).unwrap();
        simple = true;
    }

    let stdout_thread = std::thread::spawn(move || {
        let mut stdout = std::io::BufWriter::new(std::io::stdout());
        for dent in rx {
            stdout.write(&*Vec::from_path_lossy(dent.path())).unwrap();
            stdout.write(b"\n").unwrap();
        }
    });

    if parallel {
        let walker = WalkBuilder::new(path).threads(6).build_parallel();
        walker.run(|| {
            let tx = tx.clone();
            Box::new(move |result| {
                use ignore::WalkState::*;

                tx.send(DirEntry::Y(result.unwrap())).unwrap();
                Continue
            })
        });
    } else if simple {
        let walker = WalkDir::new(path);
        for result in walker {
            tx.send(DirEntry::X(result.unwrap())).unwrap();
        }
    } else {
        let walker = WalkBuilder::new(path).build();
        for result in walker {
            tx.send(DirEntry::Y(result.unwrap())).unwrap();
        }
    }
    drop(tx);
    stdout_thread.join().unwrap();
}

enum DirEntry {
    X(walkdir::DirEntry),
    Y(ignore::DirEntry),
}

impl DirEntry {
    fn path(&self) -> &Path {
        match *self {
            DirEntry::X(ref x) => x.path(),
            DirEntry::Y(ref y) => y.path(),
        }
    }
}
