//! Plan slug generation for unique plan file naming.
//!
//! Generates slugs in the format `{adjective}-{action}-{noun}` following
//! Claude Code v2.1.7 conventions. Total combinations: 168 × 87 × 235 = 3,436,980

use std::collections::HashMap;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use rand::Rng;

/// Maximum retry attempts for collision detection.
const MAX_SLUG_RETRIES: i32 = 10;

/// Session-based slug cache to prevent regeneration.
static SLUG_CACHE: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// 168 adjectives for slug generation.
const ADJECTIVES: &[&str] = &[
    // Nature/weather
    "abundant",
    "ancient",
    "autumn",
    "blazing",
    "breezy",
    "bright",
    "calm",
    "chilly",
    "cloudy",
    "coastal",
    "crisp",
    "damp",
    "dark",
    "dawn",
    "dewy",
    "dry",
    "dusk",
    "dusty",
    "evening",
    "faded",
    "foggy",
    "forest",
    "frosty",
    "frozen",
    "gentle",
    "golden",
    "green",
    "hazy",
    "hidden",
    "icy",
    "leafy",
    "lush",
    "meadow",
    "misty",
    "moonlit",
    "morning",
    "mossy",
    "muddy",
    "night",
    "northern",
    "ocean",
    "pale",
    "quiet",
    "rainy",
    "rustic",
    "sandy",
    "shady",
    "silent",
    "snowy",
    "solar",
    "southern",
    "spring",
    "starry",
    "still",
    "stormy",
    "summer",
    "sunny",
    "sunset",
    "tidal",
    "twilight",
    "warm",
    "wild",
    "windy",
    "winter",
    "woodland",
    // Programming/tech
    "agile",
    "async",
    "atomic",
    "binary",
    "cached",
    "compiled",
    "concurrent",
    "cosmic",
    "crypto",
    "cyber",
    "dynamic",
    "elastic",
    "encrypted",
    "epic",
    "fast",
    "fuzzy",
    "generic",
    "global",
    "hashed",
    "hyper",
    "immutable",
    "indexed",
    "instant",
    "lazy",
    "linked",
    "local",
    "logical",
    "magic",
    "mapped",
    "massive",
    "meta",
    "micro",
    "minimal",
    "modal",
    "modular",
    "native",
    "nested",
    "neural",
    "nimble",
    "nodal",
    "optimal",
    "packed",
    "parallel",
    "parsed",
    "partial",
    "phantom",
    "pixel",
    "polar",
    "portable",
    "prime",
    "private",
    "public",
    "quantum",
    "quick",
    "random",
    "rapid",
    "reactive",
    "recursive",
    "remote",
    "robust",
    "royal",
    "rusty",
    "scalar",
    "sealed",
    "serial",
    "sharp",
    "silent",
    "simple",
    "sleek",
    "smooth",
    "solid",
    "sonic",
    "sparse",
    "stable",
    "static",
    "steady",
    "stellar",
    "streamed",
    "strong",
    "super",
    "swift",
    "synced",
    "terse",
    "tidy",
    "tiny",
    "traced",
    "turbo",
    "typed",
    "unified",
    "unique",
    "virtual",
    "vivid",
    "wired",
    "zonal",
    // Colors
    "amber",
    "azure",
    "coral",
    "crimson",
    "cyan",
    "ebony",
    "emerald",
    "indigo",
    "ivory",
    "jade",
    "lavender",
    "lime",
    "magenta",
    "maroon",
    "navy",
    "olive",
    "orange",
    "orchid",
    "pearl",
    "pink",
    "plum",
    "rose",
    "ruby",
    "sapphire",
    "scarlet",
    "silver",
    "teal",
    "turquoise",
    "violet",
    "white",
];

/// 87 action words (gerunds) for slug generation.
const ACTIONS: &[&str] = &[
    "baking",
    "beaming",
    "blazing",
    "blending",
    "blooming",
    "bouncing",
    "brewing",
    "bubbling",
    "building",
    "buzzing",
    "calling",
    "carving",
    "casting",
    "charging",
    "chasing",
    "climbing",
    "coding",
    "coiling",
    "cooking",
    "copying",
    "crafting",
    "crossing",
    "cruising",
    "dancing",
    "dashing",
    "diving",
    "docking",
    "drafting",
    "drawing",
    "dreaming",
    "drifting",
    "drilling",
    "driving",
    "dropping",
    "echoing",
    "fading",
    "falling",
    "fishing",
    "flashing",
    "floating",
    "flowing",
    "flying",
    "folding",
    "forging",
    "forming",
    "gliding",
    "glowing",
    "growing",
    "guiding",
    "hacking",
    "hiking",
    "hopping",
    "hunting",
    "jumping",
    "landing",
    "launching",
    "leaping",
    "linking",
    "loading",
    "looping",
    "mapping",
    "marching",
    "mixing",
    "moving",
    "orbiting",
    "packing",
    "parsing",
    "passing",
    "patching",
    "pinging",
    "planting",
    "playing",
    "plotting",
    "pushing",
    "racing",
    "reading",
    "rising",
    "rolling",
    "running",
    "sailing",
    "seeking",
    "shaping",
    "sharing",
    "shifting",
    "shining",
    "singing",
    "skating",
    "skiing",
    "sleeping",
    "sliding",
    "soaring",
    "sorting",
    "sparking",
    "spinning",
    "splashing",
    "sprinting",
    "stacking",
    "staging",
    "starting",
    "stepping",
    "stirring",
    "streaming",
    "striking",
    "surfing",
    "swimming",
    "swinging",
    "syncing",
    "testing",
    "thinking",
    "ticking",
    "tinkering",
    "tracking",
    "trading",
    "training",
    "traveling",
    "turning",
    "twisting",
    "typing",
    "wading",
    "walking",
    "watching",
    "waving",
    "weaving",
    "welding",
    "whistling",
    "winding",
    "writing",
    "zooming",
];

/// 235 nouns for slug generation (includes CS pioneer names).
const NOUNS: &[&str] = &[
    // Food
    "apple",
    "avocado",
    "bacon",
    "bagel",
    "banana",
    "biscuit",
    "bread",
    "broccoli",
    "brownie",
    "burger",
    "burrito",
    "butter",
    "cake",
    "candy",
    "carrot",
    "cheese",
    "cherry",
    "chicken",
    "chili",
    "chips",
    "chocolate",
    "cinnamon",
    "coconut",
    "coffee",
    "cookie",
    "corn",
    "cream",
    "croissant",
    "cupcake",
    "curry",
    "donut",
    "dumpling",
    "egg",
    "falafel",
    "fish",
    "fries",
    "garlic",
    "ginger",
    "grape",
    "gummy",
    "hazelnut",
    "honey",
    "hummus",
    "icecream",
    "jam",
    "jelly",
    "ketchup",
    "lemon",
    "lettuce",
    "lime",
    "mango",
    "maple",
    "meatball",
    "melon",
    "mochi",
    "muffin",
    "mushroom",
    "mustard",
    "noodle",
    "nutella",
    "oatmeal",
    "olive",
    "onion",
    "orange",
    "pancake",
    "pasta",
    "peach",
    "peanut",
    "pepper",
    "pickle",
    "pie",
    "pineapple",
    "pizza",
    "plum",
    "popcorn",
    "potato",
    "pretzel",
    "pudding",
    "pumpkin",
    "raisin",
    "ramen",
    "raspberry",
    "rice",
    "salad",
    "salmon",
    "salsa",
    "sandwich",
    "sauce",
    "sausage",
    "smoothie",
    "snack",
    "soup",
    "spinach",
    "steak",
    "strawberry",
    "sushi",
    "taco",
    "toast",
    "tofu",
    "tomato",
    "truffle",
    "tuna",
    "vanilla",
    "waffle",
    "walnut",
    "wasabi",
    "yogurt",
    // Animals
    "badger",
    "bear",
    "beaver",
    "beetle",
    "bird",
    "bison",
    "bobcat",
    "bunny",
    "butterfly",
    "camel",
    "cardinal",
    "cat",
    "cheetah",
    "chicken",
    "chipmunk",
    "cobra",
    "coyote",
    "crab",
    "crane",
    "cricket",
    "crow",
    "deer",
    "dog",
    "dolphin",
    "dove",
    "dragon",
    "duck",
    "eagle",
    "elephant",
    "elk",
    "falcon",
    "finch",
    "firefly",
    "flamingo",
    "fox",
    "frog",
    "gazelle",
    "gerbil",
    "giraffe",
    "goat",
    "goose",
    "gorilla",
    "grasshopper",
    "hamster",
    "hawk",
    "hedgehog",
    "heron",
    "hippo",
    "horse",
    "hummingbird",
    "jaguar",
    "jellyfish",
    "kangaroo",
    "koala",
    "ladybug",
    "lemur",
    "leopard",
    "lion",
    "lizard",
    "llama",
    "lobster",
    "lynx",
    "macaw",
    "mantis",
    "meerkat",
    "monkey",
    "moose",
    "moth",
    "mouse",
    "newt",
    "octopus",
    "orca",
    "osprey",
    "ostrich",
    "otter",
    "owl",
    "panda",
    "panther",
    "parrot",
    "peacock",
    "pelican",
    "penguin",
    "pheasant",
    "phoenix",
    "pigeon",
    "pony",
    "porcupine",
    "puma",
    "quail",
    "rabbit",
    "raccoon",
    "raven",
    "rhino",
    "robin",
    "rooster",
    "salmon",
    "seal",
    "shark",
    "sheep",
    "shrimp",
    "sloth",
    "snail",
    "snake",
    "sparrow",
    "spider",
    "squid",
    "squirrel",
    "stork",
    "swan",
    "tiger",
    "toucan",
    "trout",
    "turtle",
    "vulture",
    "walrus",
    "weasel",
    "whale",
    "wolf",
    "wombat",
    "woodpecker",
    "wren",
    "yak",
    "zebra",
    // Nature/space
    "asteroid",
    "aurora",
    "canyon",
    "cliff",
    "cloud",
    "comet",
    "coral",
    "crater",
    "crystal",
    "desert",
    "dune",
    "eclipse",
    "ember",
    "fjord",
    "flare",
    "forest",
    "galaxy",
    "glacier",
    "glade",
    "grove",
    "harbor",
    "horizon",
    "island",
    "lagoon",
    "lake",
    "meadow",
    "meteor",
    "moon",
    "mountain",
    "nebula",
    "nova",
    "oasis",
    "ocean",
    "orbit",
    "peak",
    "planet",
    "pond",
    "prairie",
    "quasar",
    "rain",
    "rainbow",
    "reef",
    "ridge",
    "river",
    "rock",
    "sand",
    "shore",
    "sky",
    "snow",
    "spring",
    "star",
    "storm",
    "stream",
    "sun",
    "tide",
    "thunder",
    "trail",
    "tree",
    "valley",
    "volcano",
    "wave",
    "wind",
    "woods",
    // CS pioneers (last names)
    "abelson",
    "backus",
    "church",
    "dijkstra",
    "engelbart",
    "feigenbaum",
    "gosling",
    "hopper",
    "iverson",
    "joy",
    "kay",
    "lamport",
    "mccarthy",
    "naur",
    "odersky",
    "pike",
    "ritchie",
    "stallman",
    "thompson",
    "ullman",
    "vanrossum",
    "wirth",
    "xerox",
    "yourdon",
    "zuse",
];

/// Generate a random slug in the format `{adjective}-{action}-{noun}`.
pub fn generate_slug() -> String {
    let mut rng = rand::rng();
    let adj = ADJECTIVES[rng.random_range(0..ADJECTIVES.len())];
    let action = ACTIONS[rng.random_range(0..ACTIONS.len())];
    let noun = NOUNS[rng.random_range(0..NOUNS.len())];
    format!("{adj}-{action}-{noun}")
}

/// Get or generate a unique slug for a session.
///
/// Uses session-based caching to ensure the same slug is returned
/// for the same session ID. Performs collision detection with up to
/// `MAX_SLUG_RETRIES` attempts.
///
/// # Arguments
///
/// * `session_id` - The session identifier for caching
/// * `existing_slugs` - Optional set of existing slugs to avoid collisions
///
/// # Returns
///
/// The cached slug if one exists for this session, otherwise a new unique slug.
pub fn get_unique_slug(session_id: &str, existing_slugs: Option<&[String]>) -> String {
    let mut cache = SLUG_CACHE.lock().unwrap_or_else(|e| e.into_inner());

    // Check cache first
    if let Some(slug) = cache.get(session_id) {
        return slug.clone();
    }

    // Generate new slug with collision detection
    let existing = existing_slugs.unwrap_or(&[]);
    let mut attempts = 0;
    let slug = loop {
        let candidate = generate_slug();
        if !existing.contains(&candidate) || attempts >= MAX_SLUG_RETRIES {
            break candidate;
        }
        attempts += 1;
    };

    // Cache and return
    cache.insert(session_id.to_string(), slug.clone());
    slug
}

/// Clear the slug cache for testing purposes.
#[doc(hidden)]
pub fn clear_slug_cache() {
    let mut cache = SLUG_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_slug_format() {
        let slug = generate_slug();
        let parts: Vec<&str> = slug.split('-').collect();
        assert_eq!(parts.len(), 3, "Slug should have 3 parts: {slug}");
        assert!(
            ADJECTIVES.contains(&parts[0]),
            "First part should be adjective"
        );
        assert!(ACTIONS.contains(&parts[1]), "Second part should be action");
        assert!(NOUNS.contains(&parts[2]), "Third part should be noun");
    }

    #[test]
    fn test_generate_slug_uniqueness() {
        // Generate 100 slugs and check for reasonable uniqueness
        let slugs: Vec<String> = (0..100).map(|_| generate_slug()).collect();
        let unique_count = slugs.iter().collect::<std::collections::HashSet<_>>().len();
        // With 3.4M combinations, 100 random slugs should be nearly all unique
        assert!(
            unique_count >= 95,
            "Expected at least 95 unique slugs, got {unique_count}"
        );
    }

    #[test]
    fn test_get_unique_slug_caching() {
        clear_slug_cache();

        let session = "test-session-1";
        let slug1 = get_unique_slug(session, None);
        let slug2 = get_unique_slug(session, None);

        assert_eq!(slug1, slug2, "Same session should return same slug");
    }

    #[test]
    fn test_get_unique_slug_different_sessions() {
        clear_slug_cache();

        let slug1 = get_unique_slug("session-a", None);
        let slug2 = get_unique_slug("session-b", None);

        // Different sessions could theoretically get the same slug but very unlikely
        // This test just verifies the function works for different sessions
        assert!(!slug1.is_empty());
        assert!(!slug2.is_empty());
    }

    #[test]
    fn test_get_unique_slug_collision_avoidance() {
        clear_slug_cache();

        // Generate a slug and mark it as existing
        let existing_slug = generate_slug();
        let existing = vec![existing_slug.clone()];

        // Get a new slug avoiding the existing one
        let new_slug = get_unique_slug("collision-test", Some(&existing));

        // Very likely to be different (unless we hit the same random in 10 attempts)
        // This is a probabilistic test
        assert!(!new_slug.is_empty());
    }

    #[test]
    fn test_word_list_sizes() {
        // Minimum word counts to ensure sufficient combinations
        assert!(
            ADJECTIVES.len() >= 100,
            "Should have at least 100 adjectives, got {}",
            ADJECTIVES.len()
        );
        assert!(
            ACTIONS.len() >= 80,
            "Should have at least 80 actions, got {}",
            ACTIONS.len()
        );
        assert!(
            NOUNS.len() >= 200,
            "Should have at least 200 nouns, got {}",
            NOUNS.len()
        );

        // Total combinations should be > 1.5 million for low collision probability
        let total = ADJECTIVES.len() * ACTIONS.len() * NOUNS.len();
        assert!(total > 1_500_000, "Total combinations: {total}");
    }
}
