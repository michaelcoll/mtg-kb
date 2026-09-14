use std::collections::HashSet;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub struct DecklistLine {
    pub quantity: u32,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Decklist {
    pub commander: Vec<DecklistLine>,
    pub deck: Vec<DecklistLine>,
}

/// Sections reconnues (insensible à la casse) ; toute autre section est ignorée.
static COMMANDER_HEADERS: &[&str] = &["commander", "commanders"];
static DECK_HEADERS: &[&str] = &["deck", "decklist", "mainboard", "main"];

enum Section {
    Commander,
    Deck,
    Ignored,
}

static QUANTITY_LINE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(\d+)x?\s+(.+)$").unwrap());

pub fn parse(input: &str) -> Decklist {
    let mut decklist = Decklist::default();
    let mut current = Section::Ignored;
    let recognized_headers: HashSet<&str> = COMMANDER_HEADERS
        .iter()
        .chain(DECK_HEADERS.iter())
        .copied()
        .collect();

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let header_candidate = line.trim_end_matches(':').to_ascii_lowercase();
        if recognized_headers.contains(header_candidate.as_str()) {
            current = if COMMANDER_HEADERS.contains(&header_candidate.as_str()) {
                Section::Commander
            } else {
                Section::Deck
            };
            continue;
        }
        // Une ligne qui ressemble à un en-tête de section mais n'en est pas
        // une reconnue (ex. "Sideboard") bascule vers Ignored.
        if line.chars().all(|c| c.is_alphabetic() || c == ' ') && !QUANTITY_LINE.is_match(line) {
            current = Section::Ignored;
            continue;
        }

        let Some(caps) = QUANTITY_LINE.captures(line) else {
            continue;
        };
        let quantity: u32 = caps.get(1).unwrap().as_str().parse().unwrap_or(1);
        let name = caps.get(2).unwrap().as_str().trim().to_string();
        let entry = DecklistLine { quantity, name };

        match current {
            Section::Commander => decklist.commander.push(entry),
            Section::Deck => decklist.deck.push(entry),
            Section::Ignored => {}
        }
    }

    decklist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commander_and_deck_sections() {
        let input = "\
Commander
1 Atraxa, Praetors' Voice

Deck
1 Sol Ring
10 Forest
";
        let decklist = parse(input);
        assert_eq!(
            decklist.commander,
            vec![DecklistLine {
                quantity: 1,
                name: "Atraxa, Praetors' Voice".to_string()
            }]
        );
        assert_eq!(
            decklist.deck,
            vec![
                DecklistLine {
                    quantity: 1,
                    name: "Sol Ring".to_string()
                },
                DecklistLine {
                    quantity: 10,
                    name: "Forest".to_string()
                },
            ]
        );
    }

    #[test]
    fn ignores_other_sections_like_sideboard() {
        let input = "\
Commander
1 Atraxa, Praetors' Voice

Deck
1 Sol Ring

Sideboard
1 Some Sideboard Card
";
        let decklist = parse(input);
        assert_eq!(decklist.deck.len(), 1);
        assert!(
            decklist
                .deck
                .iter()
                .all(|l| l.name != "Some Sideboard Card")
        );
    }

    #[test]
    fn handles_double_faced_card_names() {
        let input = "\
Deck
1 Fable of the Mirror-Breaker // Reflection of Kiki-Jiki
";
        let decklist = parse(input);
        assert_eq!(
            decklist.deck[0].name,
            "Fable of the Mirror-Breaker // Reflection of Kiki-Jiki"
        );
    }

    #[test]
    fn accepts_x_suffix_on_quantity() {
        let input = "Deck\n2x Sol Ring\n";
        let decklist = parse(input);
        assert_eq!(decklist.deck[0].quantity, 2);
        assert_eq!(decklist.deck[0].name, "Sol Ring");
    }
}
