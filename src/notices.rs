//! Canonical generated legal notice embedded in every release adapter.

pub const THIRD_PARTY_NOTICES: &str = include_str!("../THIRD_PARTY_NOTICES.txt");

#[cfg(test)]
mod tests {
    use super::THIRD_PARTY_NOTICES;

    const RELEASE_SBOM: &str = include_str!("../SBOM.cdx.json");

    #[test]
    fn canonical_notice_covers_project_authority_and_release_graph() {
        assert!(THIRD_PARTY_NOTICES.contains("Volmap Inspector is licensed under Apache-2.0"));
        assert!(THIRD_PARTY_NOTICES.contains("e1e651debf6cc100172bde96603b17424f9c135a"));
        assert!(THIRD_PARTY_NOTICES.contains("aes 0.9.2"));
        assert!(THIRD_PARTY_NOTICES.contains("aria 0.2.0"));
        assert!(THIRD_PARTY_NOTICES.contains("react 19.2.8"));
        assert!(THIRD_PARTY_NOTICES.contains("react-dom 19.2.8"));
        assert!(RELEASE_SBOM.contains("pkg:npm/react@19.2.8"));
        assert!(RELEASE_SBOM.contains("pkg:npm/react-dom@19.2.8"));
        assert!(
            THIRD_PARTY_NOTICES.contains("Recovered artifacts are neither linked nor distributed")
        );
    }
}
