//! Cache EDHREC (voir ADR 0003) : une réponse par slug de Commandant, sous
//! `<cache_dir>/<slug>.json`, réutilisée tant qu'elle a moins de `ttl`.
//! Recommander n'a pas de cache : sa réponse dépend de la Decklist complète.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};

use super::edhrec::EdhrecClient;

/// Sert le contenu en cache pour `slug` s'il a moins de `ttl`, sinon
/// interroge `client` et rafraîchit le cache. La fraîcheur est décidée par
/// la date de modification du fichier (`mtime`), plus simple qu'un horodatage
/// embarqué dans le JSON mis en cache.
pub fn cached_fetch(
    client: &dyn EdhrecClient,
    cache_dir: &Path,
    slug: &str,
    ttl: Duration,
) -> Result<String> {
    let path = cache_path(cache_dir, slug);
    if let Some(content) = read_if_fresh(&path, ttl) {
        return Ok(content);
    }

    let body = client.fetch(slug)?;
    std::fs::create_dir_all(cache_dir)
        .with_context(|| format!("création du cache {}", cache_dir.display()))?;
    std::fs::write(&path, &body)
        .with_context(|| format!("écriture du cache {}", path.display()))?;
    Ok(body)
}

fn cache_path(cache_dir: &Path, slug: &str) -> PathBuf {
    cache_dir.join(format!("{slug}.json"))
}

fn read_if_fresh(path: &Path, ttl: Duration) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let age = modified.elapsed().ok()?;
    if age >= ttl {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::time::{Duration, SystemTime};

    struct CountingClient {
        calls: Cell<u32>,
        response: String,
    }

    impl EdhrecClient for CountingClient {
        fn fetch(&self, _slug: &str) -> Result<String> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.response.clone())
        }
    }

    #[test]
    fn fetches_from_the_network_when_no_cache_exists() {
        let dir = tempfile::tempdir().unwrap();
        let client = CountingClient {
            calls: Cell::new(0),
            response: "{}".to_string(),
        };
        let body = cached_fetch(
            &client,
            dir.path(),
            "atraxa",
            Duration::from_secs(7 * 86400),
        )
        .unwrap();
        assert_eq!(body, "{}");
        assert_eq!(client.calls.get(), 1);
    }

    #[test]
    fn reuses_a_fresh_cache_without_calling_the_network() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("atraxa.json"), "cached-body").unwrap();
        let client = CountingClient {
            calls: Cell::new(0),
            response: "fresh-body".to_string(),
        };
        let body = cached_fetch(
            &client,
            dir.path(),
            "atraxa",
            Duration::from_secs(7 * 86400),
        )
        .unwrap();
        assert_eq!(body, "cached-body");
        assert_eq!(client.calls.get(), 0, "must not call the network");
    }

    #[test]
    fn refreshes_a_stale_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atraxa.json");
        std::fs::write(&path, "stale-body").unwrap();
        // Recule la date de modification de 8 jours pour simuler un cache
        // expiré (TTL de 7 jours).
        let stale_time = SystemTime::now() - Duration::from_secs(8 * 86400);
        let file = std::fs::File::open(&path).unwrap();
        file.set_modified(stale_time).unwrap();

        let client = CountingClient {
            calls: Cell::new(0),
            response: "fresh-body".to_string(),
        };
        let body = cached_fetch(
            &client,
            dir.path(),
            "atraxa",
            Duration::from_secs(7 * 86400),
        )
        .unwrap();
        assert_eq!(body, "fresh-body");
        assert_eq!(client.calls.get(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "fresh-body");
    }

    #[test]
    fn different_slugs_get_different_cache_entries() {
        let dir = tempfile::tempdir().unwrap();
        let client_a = CountingClient {
            calls: Cell::new(0),
            response: "body-a".to_string(),
        };
        let client_b = CountingClient {
            calls: Cell::new(0),
            response: "body-b".to_string(),
        };
        let body_a = cached_fetch(
            &client_a,
            dir.path(),
            "atraxa",
            Duration::from_secs(7 * 86400),
        )
        .unwrap();
        let body_b = cached_fetch(
            &client_b,
            dir.path(),
            "krenko-mob-boss",
            Duration::from_secs(7 * 86400),
        )
        .unwrap();
        assert_eq!(body_a, "body-a");
        assert_eq!(body_b, "body-b");
    }
}
