//! Which language the interface is presented in.
//!
//! Here rather than beside the strings themselves because a language is a
//! *preference*, like the display unit next door — the interface reads it, the
//! command that changes it is a command like any other, and a View may not own
//! something a Command has to carry.

/// Which language the interface is presented in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    /// Brazilian Portuguese — the design's own language.
    PtBr,
    /// The default, which is not the design's own language and is deliberate:
    /// the interface has to open in something a first-time user can read, and
    /// English is the one this application's audience is most likely to share.
    /// A system tag still wins over it — see [`Locale::from_tag`] — and the
    /// choice is a setting, so nobody is stuck with it.
    #[default]
    EnUs,
    /// Latin American Spanish rather than Castilian: the design is Brazilian,
    /// so the market next door is the one this reaches first, and its
    /// vocabulary is the one those artists already use.
    Es419,
}

impl Locale {
    pub const ALL: [Locale; 3] = [Self::PtBr, Self::EnUs, Self::Es419];

    /// Picks a locale from a system tag, falling back to the default.
    ///
    /// A tag with no translation gets the default rather than untranslated
    /// keys, which is the difference between an interface in the wrong
    /// language and one that is broken.
    pub fn from_tag(tag: &str) -> Self {
        let tag = tag.to_ascii_lowercase();
        if tag.starts_with("pt") {
            Self::PtBr
        } else if tag.starts_with("en") {
            Self::EnUs
        } else if tag.starts_with("es") {
            // Every Spanish tag, Castilian included: a Madrid interface in
            // Latin American Spanish is read; one in Portuguese is not.
            Self::Es419
        } else {
            Self::default()
        }
    }

    /// The tag this locale is stored and recognised under.
    ///
    /// Round-trips with [`Locale::from_tag`], so what is written to the
    /// session and what is read back cannot drift apart.
    pub fn tag(self) -> &'static str {
        match self {
            Self::PtBr => "pt-BR",
            Self::EnUs => "en-US",
            Self::Es419 => "es-419",
        }
    }

    /// How the language menu names it.
    ///
    /// In the language *itself*, which is the one rule a language menu has: a
    /// reader who cannot read the current interface can still find their own.
    pub fn label(self) -> &'static str {
        match self {
            Self::PtBr => "Português (Brasil)",
            Self::EnUs => "English (US)",
            Self::Es419 => "Español (Latinoamérica)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_reads_back_as_the_locale_that_wrote_it() {
        // The session stores a tag, so a locale that did not round-trip would
        // open in a different language from the one that was chosen.
        for locale in Locale::ALL {
            assert_eq!(
                Locale::from_tag(locale.tag()),
                locale,
                "{} did not read back",
                locale.tag()
            );
        }
    }

    #[test]
    fn a_system_tag_is_recognised_by_its_language() {
        // Region and case are not the question: what matters is which of the
        // three translations a reader gets.
        assert_eq!(Locale::from_tag("pt_PT.UTF-8"), Locale::PtBr);
        assert_eq!(Locale::from_tag("PT-br"), Locale::PtBr);
        assert_eq!(Locale::from_tag("en_GB"), Locale::EnUs);
        // Castilian included: a Madrid interface in Latin American Spanish is
        // read; one in Portuguese is not.
        assert_eq!(Locale::from_tag("es_ES"), Locale::Es419);
    }

    #[test]
    fn an_unknown_tag_opens_in_english() {
        // The default rather than untranslated keys, which is the difference
        // between an interface in the wrong language and one that is broken.
        assert_eq!(Locale::from_tag("ja-JP"), Locale::EnUs);
        assert_eq!(Locale::from_tag(""), Locale::EnUs);
        assert_eq!(Locale::default(), Locale::EnUs);
    }

    #[test]
    fn every_language_names_itself() {
        for locale in Locale::ALL {
            assert!(
                !locale.label().is_empty(),
                "{} has no name in the menu",
                locale.tag()
            );
        }
        assert!(Locale::EnUs.label().contains("English"));
        assert!(Locale::PtBr.label().contains("Português"));
        assert!(Locale::Es419.label().contains("Español"));
    }
}
