use std::cmp::Ordering;

pub fn sort_desc_by_score_then_name<T>(
    items: &mut [T],
    score: impl Fn(&T) -> f64,
    name: impl Fn(&T) -> &str,
) {
    items.sort_by(|a, b| {
        score(b)
            .partial_cmp(&score(a))
            .unwrap_or(Ordering::Equal)
            .then_with(|| name(a).cmp(name(b)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_by_score_descending_then_name_ascending() {
        let mut items = vec![("Beta", 1.0), ("Alpha", 2.0), ("Gamma", 2.0)];
        sort_desc_by_score_then_name(&mut items, |i| i.1, |i| i.0);
        assert_eq!(
            items,
            vec![("Alpha", 2.0), ("Gamma", 2.0), ("Beta", 1.0)],
            "equal scores break ties by name ascending"
        );
    }
}
