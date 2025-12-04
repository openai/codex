use super::random::Rand;

#[derive(Debug)]
pub struct Edit {
    pub position: usize,
    pub deleted_length: usize,
    pub inserted_text: Vec<u8>,
}

#[must_use]
pub fn invert_edit(input: &[u8], edit: &Edit) -> Edit {
    let position = edit.position;
    let removed_content = &input[position..(position + edit.deleted_length)];
    Edit {
        position,
        deleted_length: edit.inserted_text.len(),
        inserted_text: removed_content.to_vec(),
    }
}

pub fn get_random_edit(rand: &mut Rand, input: &[u8]) -> Edit {
    let choice = rand.unsigned(10);
    if choice < 2 {
        // Insert text at end
        let inserted_text = rand.words(3);
        Edit {
            position: input.len(),
            deleted_length: 0,
            inserted_text,
        }
    } else if choice < 5 {
        // Delete text from the end
        let deleted_length = rand.unsigned(30).min(input.len());
        Edit {
            position: input.len() - deleted_length,
            deleted_length,
            inserted_text: vec![],
        }
    } else if choice < 8 {
        // Insert at a random position
        let position = rand.unsigned(input.len());
        let word_count = 1 + rand.unsigned(3);
        let inserted_text = rand.words(word_count);
        Edit {
            position,
            deleted_length: 0,
            inserted_text,
        }
    } else {
        // Replace at random position
        let position = rand.unsigned(input.len());
        let deleted_length = rand.unsigned(input.len() - position);
        let word_count = 1 + rand.unsigned(3);
        let inserted_text = rand.words(word_count);
        Edit {
            position,
            deleted_length,
            inserted_text,
        }
    }
}
