//! Four check words from two devices' fingerprints. Both Fuselanes compute the
//! same words; if the screens show different ones, something sits in between.

use sha2::Digest;

/// 256 short, plain, distinct words: one per byte.
pub const WORDS: [&str; 256] = [
    "acorn", "actor", "adobe", "agent", "alarm", "album", "alert", "alley", "amber", "angle",
    "anvil", "apple", "apron", "arena", "arrow", "aspen", "atlas", "attic", "audio", "avocado",
    "badge", "bagel", "baker", "bamboo", "banjo", "barrel", "basil", "basin", "beach", "beacon",
    "berry", "bison", "blade", "blanket", "blossom", "board", "bonus", "boulder", "bracket",
    "brick", "bridge", "bronze", "brook", "brush", "bubble", "bucket", "buffalo", "bundle",
    "burger", "butter", "cabin", "cable", "cactus", "camel", "camera", "canal", "candle", "canoe",
    "canvas", "canyon", "carbon", "cargo", "carpet", "carrot", "castle", "cedar", "cello", "chalk",
    "cherry", "chess", "chimney", "cinema", "circle", "citrus", "clay", "cliff", "clock", "cloud",
    "clover", "coast", "cobalt", "cocoa", "comet", "compass", "copper", "coral", "cotton",
    "cougar", "crane", "crayon", "creek", "cricket", "crystal", "cumin", "curtain", "cushion",
    "daisy", "dancer", "delta", "denim", "desert", "diamond", "dinner", "dolphin", "domino",
    "donkey", "dragon", "drum", "eagle", "easel", "echo", "elbow", "ember", "emerald", "engine",
    "falcon", "feather", "fern", "fiddle", "field", "flame", "flute", "forest", "fossil",
    "fountain", "fox", "galaxy", "garden", "garlic", "gecko", "ginger", "glacier", "globe",
    "goose", "granite", "grape", "gravel", "guitar", "hammer", "harbor", "harvest", "hazel",
    "helmet", "heron", "honey", "horizon", "iceberg", "igloo", "island", "ivory", "jacket",
    "jaguar", "jasmine", "jelly", "jungle", "kayak", "kettle", "kiwi", "koala", "ladder", "lagoon",
    "lantern", "lava", "lemon", "lentil", "lilac", "linen", "lion", "lizard", "lobster", "locket",
    "lotus", "magnet", "mango", "maple", "marble", "meadow", "melon", "meteor", "mint", "mirror",
    "mitten", "monkey", "mosaic", "mountain", "muffin", "nectar", "needle", "nickel", "noodle",
    "nutmeg", "oasis", "ocean", "olive", "onion", "opal", "orange", "orbit", "orchid", "otter",
    "oyster", "paddle", "palace", "panda", "panther", "paper", "parrot", "peach", "pearl",
    "pebble", "pelican", "pepper", "piano", "pigeon", "pillow", "pine", "planet", "plum", "pocket",
    "pony", "poppy", "potato", "prism", "pumpkin", "puzzle", "quartz", "quilt", "rabbit", "radar",
    "raven", "reef", "ribbon", "river", "robin", "rocket", "saddle", "saffron", "salmon", "sandal",
    "satin", "scarf", "shell", "silver", "sketch", "sparrow", "spider", "spoon", "spruce",
    "squash", "summit", "sunset", "swan", "tango", "teapot", "thunder", "tiger",
];

/// The four words for a connection between two devices. Order doesn't matter:
/// both sides sort the fingerprints first.
pub fn check_words(a: &str, b: &str) -> [&'static str; 4] {
    let (a, b) = (a.to_ascii_uppercase(), b.to_ascii_uppercase());
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let mut h = sha2::Sha256::new();
    h.update(b"fuselane nearby check words v1\n");
    h.update(lo.as_bytes());
    h.update(b"\n");
    h.update(hi.as_bytes());
    let d = h.finalize();
    [
        WORDS[d[0] as usize],
        WORDS[d[1] as usize],
        WORDS[d[2] as usize],
        WORDS[d[3] as usize],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_256_distinct_words() {
        let mut seen = std::collections::HashSet::new();
        for w in WORDS {
            assert!(seen.insert(w), "{w} twice");
            assert!(w.chars().all(|c| c.is_ascii_lowercase()));
        }
        assert_eq!(seen.len(), 256);
    }

    #[test]
    fn both_sides_get_the_same_words_and_others_get_different_ones() {
        let (a, b) = ("AB12", "cd34");
        assert_eq!(check_words(a, b), check_words(b, a));
        assert_eq!(check_words(a, b), check_words("ab12", "CD34"));
        assert_ne!(check_words(a, b), check_words(a, "CD35"));
    }
}
