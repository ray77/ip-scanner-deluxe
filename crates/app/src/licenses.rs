//! Copyright and license texts shown in the About dialog. They are embedded
//! so they stay reachable in every distribution, including the MSIX package.

pub const COPYRIGHT: &str = "Copyright © 2026 Dominik Schmidt";
pub const LICENSE_EXPR: &str = "MIT OR Apache-2.0";
pub const SOURCE_URL: &str = env!("CARGO_PKG_REPOSITORY");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseDoc {
    Mit,
    Apache,
    ThirdParty,
}

impl LicenseDoc {
    pub fn title(self) -> &'static str {
        match self {
            LicenseDoc::Mit => "MIT License",
            LicenseDoc::Apache => "Apache License 2.0",
            LicenseDoc::ThirdParty => "Third-Party Licenses",
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            LicenseDoc::Mit => include_str!("../../../LICENSE-MIT"),
            LicenseDoc::Apache => include_str!("../../../LICENSE-APACHE"),
            LicenseDoc::ThirdParty => include_str!("../../../THIRD-PARTY-NOTICES.txt"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_are_embedded() {
        assert!(LicenseDoc::Mit.text().contains("Dominik Schmidt"));
        assert!(LicenseDoc::Apache.text().contains("Apache License"));
        let notices = LicenseDoc::ThirdParty.text();
        assert!(notices.contains("IEEE Registration Authority"));
        assert!(notices.contains("Used by:"));
        assert_eq!(SOURCE_URL, "https://github.com/ray77/ip-scanner-deluxe");
    }
}
