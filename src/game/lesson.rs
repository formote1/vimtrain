//! Chapters. Fine-grained progression lives in curriculum (skills);
//! a chapter is just a named bundle of skills 1-6 for menu + headers.

pub struct Lesson {
    pub id: u8,
    pub name: &'static str,
    #[allow(dead_code)]
    pub tagline: &'static str,
    pub keys: &'static str,
}

pub const LESSONS: [Lesson; 6] = [
    Lesson {
        id: 1,
        name: "Baby Steps",
        tagline: "The core: move one cell at a time.",
        keys: "h j k l",
    },
    Lesson {
        id: 2,
        name: "Word Hopper",
        tagline: "Jump by words, not characters.",
        keys: "w W b B e E",
    },
    Lesson {
        id: 3,
        name: "Line Sniper",
        tagline: "Jump straight to a character on the line.",
        keys: "f F t T ; ,",
    },
    Lesson {
        id: 4,
        name: "Line Surfer",
        tagline: "Line edges and file jumps.",
        keys: "0 ^ $ g_ gg G H M L",
    },
    Lesson {
        id: 5,
        name: "Power Moves",
        tagline: "Paragraphs and counts multiply everything.",
        keys: "{ } + counts (3j, 2w, 5G ...)",
    },
    Lesson {
        id: 6,
        name: "Free Practice",
        tagline: "Everything at once. Chase par.",
        keys: "all motions",
    },
];
