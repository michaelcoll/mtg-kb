use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::model::Face;

/// Types de créature de la règle 205.3m des Comprehensive Rules.
const CREATURE_TYPES: &[&str] = &[
    "Advisor",
    "Aetherborn",
    "Alien",
    "Ally",
    "Angel",
    "Antelope",
    "Ape",
    "Archer",
    "Archon",
    "Armadillo",
    "Army",
    "Artificer",
    "Assassin",
    "Assembly-Worker",
    "Astartes",
    "Atog",
    "Aurochs",
    "Avatar",
    "Azra",
    "Badger",
    "Balloon",
    "Barbarian",
    "Bard",
    "Basilisk",
    "Bat",
    "Bear",
    "Beast",
    "Beaver",
    "Beeble",
    "Beholder",
    "Berserker",
    "Bird",
    "Bison",
    "Blinkmoth",
    "Boar",
    "Bringer",
    "Brushwagg",
    "Camarid",
    "Camel",
    "Capybara",
    "Caribou",
    "Carrier",
    "Cat",
    "Centaur",
    "Child",
    "Chimera",
    "Citizen",
    "Cleric",
    "Clown",
    "Cockatrice",
    "Construct",
    "Coward",
    "Coyote",
    "Crab",
    "Crocodile",
    "C’tan",
    "Custodes",
    "Cyberman",
    "Cyclops",
    "Dalek",
    "Dauthi",
    "Demigod",
    "Demon",
    "Deserter",
    "Detective",
    "Devil",
    "Dinosaur",
    "Djinn",
    "Doctor",
    "Dog",
    "Dragon",
    "Drake",
    "Dreadnought",
    "Drix",
    "Drone",
    "Druid",
    "Dryad",
    "Dwarf",
    "Echidna",
    "Efreet",
    "Egg",
    "Elder",
    "Eldrazi",
    "Elemental",
    "Elephant",
    "Elf",
    "Elk",
    "Employee",
    "Eye",
    "Faerie",
    "Ferret",
    "Fish",
    "Flagbearer",
    "Fox",
    "Fractal",
    "Frog",
    "Fungus",
    "Gamer",
    "Gargoyle",
    "Germ",
    "Giant",
    "Gith",
    "Glimmer",
    "Gnoll",
    "Gnome",
    "Goat",
    "Goblin",
    "God",
    "Golem",
    "Gorgon",
    "Graveborn",
    "Gremlin",
    "Griffin",
    "Guest",
    "Hag",
    "Halfling",
    "Hamster",
    "Harpy",
    "Hedgehog",
    "Hellion",
    "Hero",
    "Hippo",
    "Hippogriff",
    "Homarid",
    "Homunculus",
    "Horror",
    "Horse",
    "Human",
    "Hydra",
    "Hyena",
    "Illusion",
    "Imp",
    "Incarnation",
    "Inkling",
    "Inquisitor",
    "Insect",
    "Jackal",
    "Jellyfish",
    "Juggernaut",
    "Kangaroo",
    "Kavu",
    "Kirin",
    "Kithkin",
    "Knight",
    "Kobold",
    "Kor",
    "Kraken",
    "Llama",
    "Lamia",
    "Lammasu",
    "Leech",
    "Lemur",
    "Leviathan",
    "Lhurgoyf",
    "Licid",
    "Lizard",
    "Lobster",
    "Manticore",
    "Masticore",
    "Mercenary",
    "Merfolk",
    "Metathran",
    "Minion",
    "Minotaur",
    "Mite",
    "Mole",
    "Monger",
    "Mongoose",
    "Monk",
    "Monkey",
    "Moogle",
    "Moonfolk",
    "Mount",
    "Mouse",
    "Mutant",
    "Myr",
    "Mystic",
    "Nautilus",
    "Necron",
    "Nephilim",
    "Nightmare",
    "Nightstalker",
    "Ninja",
    "Noble",
    "Noggle",
    "Nomad",
    "Nymph",
    "Octopus",
    "Ogre",
    "Ooze",
    "Orb",
    "Orc",
    "Orgg",
    "Otter",
    "Ouphe",
    "Ox",
    "Oyster",
    "Pangolin",
    "Peasant",
    "Pegasus",
    "Pentavite",
    "Performer",
    "Pest",
    "Phelddagrif",
    "Phoenix",
    "Phyrexian",
    "Pilot",
    "Pincher",
    "Pirate",
    "Plant",
    "Platypus",
    "Porcupine",
    "Possum",
    "Praetor",
    "Primarch",
    "Prism",
    "Processor",
    "Qu",
    "Rabbit",
    "Raccoon",
    "Ranger",
    "Rat",
    "Rebel",
    "Reflection",
    "Rhino",
    "Rigger",
    "Robot",
    "Rogue",
    "Sable",
    "Salamander",
    "Samurai",
    "Sand",
    "Saproling",
    "Satyr",
    "Scarecrow",
    "Scientist",
    "Scion",
    "Scorpion",
    "Scout",
    "Sculpture",
    "Seal",
    "Serf",
    "Serpent",
    "Servo",
    "Shade",
    "Shaman",
    "Shapeshifter",
    "Shark",
    "Sheep",
    "Siren",
    "Skeleton",
    "Skunk",
    "Slith",
    "Sliver",
    "Sloth",
    "Slug",
    "Snail",
    "Snake",
    "Soldier",
    "Soltari",
    "Sorcerer",
    "Spawn",
    "Specter",
    "Spellshaper",
    "Sphinx",
    "Spider",
    "Spike",
    "Spirit",
    "Splinter",
    "Sponge",
    "Squid",
    "Squirrel",
    "Starfish",
    "Surrakar",
    "Survivor",
    "Symbiote",
    "Synth",
    "Tentacle",
    "Tetravite",
    "Thalakos",
    "Thopter",
    "Thrull",
    "Tiefling",
    "Time Lord",
    "Toy",
    "Treefolk",
    "Trilobite",
    "Triskelavite",
    "Troll",
    "Turtle",
    "Tyranid",
    "Unicorn",
    "Utrom",
    "Vampire",
    "Varmint",
    "Vedalken",
    "Villain",
    "Volver",
    "Wall",
    "Walrus",
    "Warlock",
    "Warrior",
    "Weasel",
    "Weird",
    "Werewolf",
    "Whale",
    "Wizard",
    "Wolf",
    "Wolverine",
    "Wombat",
    "Worm",
    "Wraith",
    "Wurm",
    "Yeti",
    "Zombie",
    "Zubera",
];

/// Pluriels irréguliers (ou invariables) des types de créature courants.
const IRREGULAR_PLURALS: &[(&str, &str)] = &[
    ("Dwarf", "Dwarves"),
    ("Elf", "Elves"),
    ("Werewolf", "Werewolves"),
    ("Wolf", "Wolves"),
    ("Mouse", "Mice"),
    ("Ox", "Oxen"),
    ("Cyclops", "Cyclopes"),
    ("Homunculus", "Homunculi"),
    ("Pegasus", "Pegasi"),
];

fn plural(creature_type: &str) -> String {
    if let Some((_, irregular)) = IRREGULAR_PLURALS
        .iter()
        .find(|(singular, _)| *singular == creature_type)
    {
        return irregular.to_string();
    }
    let ends_with_consonant_y = creature_type.ends_with('y')
        && !creature_type[..creature_type.len() - 1].ends_with(['a', 'e', 'i', 'o', 'u']);
    if ends_with_consonant_y {
        format!("{}ies", &creature_type[..creature_type.len() - 1])
    } else if ["s", "x", "ch", "sh"]
        .iter()
        .any(|end| creature_type.ends_with(end))
    {
        format!("{creature_type}es")
    } else {
        format!("{creature_type}s")
    }
}

/// Forme écrite (singulier ou pluriel) → type de créature.
static FORMS: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
    CREATURE_TYPES
        .iter()
        .flat_map(|t| [(t.to_string(), *t), (plural(t), *t)])
        .collect()
});

static NAMED_TYPE: LazyLock<Regex> = LazyLock::new(|| {
    let mut forms: Vec<&String> = FORMS.keys().collect();
    // Les formes les plus longues d'abord : "Elves" avant "Elf".
    forms.sort_by_key(|f| std::cmp::Reverse(f.len()));
    let alternation: Vec<String> = forms.iter().map(|f| regex::escape(f)).collect();
    Regex::new(&format!(r"\b(?:{})\b", alternation.join("|"))).unwrap()
});

static REMINDER_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)").unwrap());

/// Types de créature que le texte oracle de la Face nomme (« Goblins you
/// control », « other Elf creatures »), au singulier ou au pluriel, dans
/// l'ordre de première apparition. Sont ignorés : le texte de rappel, le nom
/// de la Face elle-même et les négations (« non-Human »).
pub fn named_creature_types(face: &Face) -> Vec<&'static str> {
    let Some(text) = face.oracle_text.as_deref() else {
        return Vec::new();
    };
    let text = REMINDER_TEXT.replace_all(text, "");
    let text = if face.name.is_empty() {
        text
    } else {
        text.replace(&face.name, "").into()
    };

    let mut named: Vec<&'static str> = Vec::new();
    for found in NAMED_TYPE.find_iter(&text) {
        if text[..found.start()].ends_with("non-") {
            continue;
        }
        let creature_type = FORMS[found.as_str()];
        if !named.contains(&creature_type) {
            named.push(creature_type);
        }
    }
    named
}
