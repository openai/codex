//! Built-in pet catalog ported from the Codex App avatar catalog.

pub(super) const DEFAULT_FRAME_WIDTH: u32 = 192;
pub(super) const DEFAULT_FRAME_HEIGHT: u32 = 208;
pub(super) const DEFAULT_FRAME_COLUMNS: u32 = 8;
pub(super) const DEFAULT_FRAME_ROWS: u32 = 9;
pub(super) const SPRITESHEET_WIDTH: u32 = DEFAULT_FRAME_WIDTH * DEFAULT_FRAME_COLUMNS;
pub(super) const SPRITESHEET_HEIGHT: u32 = DEFAULT_FRAME_HEIGHT * DEFAULT_FRAME_ROWS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BuiltinPet {
    pub(super) id: &'static str,
    pub(super) display_name: &'static str,
    pub(super) description: &'static str,
    pub(super) spritesheet_file: &'static str,
}

pub(super) const BUILTIN_PETS: &[BuiltinPet] = &[
    BuiltinPet {
        id: "codex",
        display_name: "Codex",
        description: "The original Codex companion",
        spritesheet_file: "codex-spritesheet-v4.webp",
    },
    BuiltinPet {
        id: "dewey",
        display_name: "Dewey",
        description: "A tidy duck for calm workspace days",
        spritesheet_file: "dewey-spritesheet-v4.webp",
    },
    BuiltinPet {
        id: "fireball",
        display_name: "Fireball",
        description: "Hot path energy for fast iteration",
        spritesheet_file: "fireball-spritesheet-v4.webp",
    },
    BuiltinPet {
        id: "rocky",
        display_name: "Rocky",
        description: "A steady rock when the diff gets large",
        spritesheet_file: "rocky-spritesheet-v4.webp",
    },
    BuiltinPet {
        id: "seedy",
        display_name: "Seedy",
        description: "Small green shoots for new ideas",
        spritesheet_file: "seedy-spritesheet-v4.webp",
    },
    BuiltinPet {
        id: "stacky",
        display_name: "Stacky",
        description: "A balanced stack for deep work",
        spritesheet_file: "stacky-spritesheet-v4.webp",
    },
    BuiltinPet {
        id: "bsod",
        display_name: "BSOD",
        description: "A tiny blue-screen gremlin",
        spritesheet_file: "bsod-spritesheet-v4.webp",
    },
    BuiltinPet {
        id: "null-signal",
        display_name: "Null Signal",
        description: "Quiet signal from the void",
        spritesheet_file: "null-signal-spritesheet-v4.webp",
    },
];

pub(super) fn builtin_pet(id: &str) -> Option<BuiltinPet> {
    BUILTIN_PETS.iter().copied().find(|pet| pet.id == id)
}

#[cfg(test)]
pub(super) fn write_test_spritesheet(path: &std::path::Path) {
    let image = image::RgbaImage::new(SPRITESHEET_WIDTH, SPRITESHEET_HEIGHT);
    image.save(path).unwrap();
}
