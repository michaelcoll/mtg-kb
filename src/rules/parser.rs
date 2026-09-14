use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub struct SectionEntry {
    pub number: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleEntry {
    pub number: String,
    pub parent: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlossaryEntry {
    pub term: String,
    pub definition: String,
}

/// Ex. "100.1", "100.1a".
static RULE_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{3}\.\d+[a-z]?)\.?\s+(.+)$").unwrap());

/// Ex. "100. General".
static SECTION_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{3})\.\s+(.+)$").unwrap());

pub fn parse_effective_date(text: &str) -> Option<String> {
    let re = Regex::new(r"effective as of (.+?)\.").unwrap();
    re.captures(text)
        .map(|c| c.get(1).unwrap().as_str().to_string())
}

fn parent_of(number: &str) -> Option<String> {
    if number.ends_with(|c: char| c.is_ascii_lowercase()) {
        Some(number[..number.len() - 1].to_string())
    } else {
        number.rfind('.').map(|pos| number[..pos].to_string())
    }
}

pub fn parse_rules(body: &str) -> (Vec<SectionEntry>, Vec<RuleEntry>) {
    let mut sections = Vec::new();
    let mut rules: Vec<RuleEntry> = Vec::new();

    for line in body.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if let Some(caps) = RULE_LINE.captures(line) {
            let number = caps.get(1).unwrap().as_str().to_string();
            let text = caps.get(2).unwrap().as_str().trim().to_string();
            rules.push(RuleEntry {
                parent: parent_of(&number),
                number,
                text,
            });
        } else if let Some(caps) = SECTION_LINE.captures(line) {
            sections.push(SectionEntry {
                number: caps.get(1).unwrap().as_str().to_string(),
                title: caps.get(2).unwrap().as_str().trim().to_string(),
            });
        } else if let Some(last) = rules.last_mut() {
            last.text.push(' ');
            last.text.push_str(line.trim());
        }
    }

    (sections, rules)
}

/// Blocs séparés par une ligne vide : terme puis définition.
pub fn parse_glossary(body: &str) -> Vec<GlossaryEntry> {
    let mut entries = Vec::new();
    let mut lines = body.lines().peekable();

    while let Some(line) = lines.next() {
        let term = line.trim();
        if term.is_empty() {
            continue;
        }
        let mut definition_lines = Vec::new();
        while let Some(next) = lines.peek() {
            if next.trim().is_empty() {
                lines.next();
                break;
            }
            definition_lines.push(lines.next().unwrap().trim().to_string());
        }
        if definition_lines.is_empty() {
            continue;
        }
        entries.push(GlossaryEntry {
            term: term.to_string(),
            definition: definition_lines.join(" "),
        });
    }

    entries
}

/// Utilise les dernières occurrences de "Glossary"/"Credits" : le sommaire
/// répète ces titres.
pub fn split_document(full_text: &str) -> (&str, &str) {
    let body_end = full_text.rfind("\nGlossary\n").unwrap_or(full_text.len());
    let glossary_start = body_end + "\nGlossary\n".len();
    let credits_start = full_text
        .rfind("\nCredits\n")
        .filter(|&pos| pos >= glossary_start)
        .unwrap_or(full_text.len());

    (
        &full_text[..body_end],
        &full_text[glossary_start..credits_start],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Magic: The Gathering Comprehensive Rules

These rules are effective as of February 27, 2026.

Introduction

1. Game Concepts

100. General

100.1. These Magic rules apply to any Magic game with two or more players,
including two-player games and multiplayer games.

100.1a A two-player game is a game that begins with only two players.

100.1b A multiplayer game is a game that begins with more than two players.
See section 8, \"Multiplayer Rules.\"

100.2. To play, each player needs their own deck of traditional Magic cards.

Glossary

Ability
A characteristic a card or ability grants a player, or an activated or
triggered ability of a permanent.

Ability Word
A word with no rules meaning that appears at the start of some abilities.

Credits

...
";

    #[test]
    fn extracts_effective_date() {
        assert_eq!(
            parse_effective_date(SAMPLE),
            Some("February 27, 2026".to_string())
        );
    }

    #[test]
    fn splits_body_and_glossary_from_credits() {
        let (body, glossary) = split_document(SAMPLE);
        assert!(body.contains("100.1a"));
        assert!(!body.contains("Ability Word"));
        assert!(glossary.contains("Ability Word"));
        assert!(!glossary.contains("Credits"));
    }

    #[test]
    fn parses_sections_and_rules_with_parents() {
        let (body, _) = split_document(SAMPLE);
        let (sections, rules) = parse_rules(body);

        assert_eq!(
            sections.last(),
            Some(&SectionEntry {
                number: "100".to_string(),
                title: "General".to_string(),
            })
        );

        let rule_100_1 = rules.iter().find(|r| r.number == "100.1").unwrap();
        assert_eq!(rule_100_1.parent, Some("100".to_string()));
        assert!(rule_100_1.text.starts_with("These Magic rules apply"));
        assert!(rule_100_1.text.contains("multiplayer games."));

        let rule_100_1a = rules.iter().find(|r| r.number == "100.1a").unwrap();
        assert_eq!(rule_100_1a.parent, Some("100.1".to_string()));
        assert!(rule_100_1a.text.starts_with("A two-player game"));

        let rule_100_1b = rules.iter().find(|r| r.number == "100.1b").unwrap();
        assert!(rule_100_1b.text.contains("Multiplayer Rules"));
    }

    #[test]
    fn parses_glossary_entries() {
        let (_, glossary_body) = split_document(SAMPLE);
        let entries = parse_glossary(glossary_body);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].term, "Ability");
        assert!(entries[0].definition.contains("triggered ability"));
        assert_eq!(entries[1].term, "Ability Word");
    }
}
