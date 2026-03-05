//! Porter Stemming Algorithm implementation for English text.
//!
//! The Porter stemmer is a suffix-stripping algorithm that reduces words to their
//! root form (stem) for better text matching in full-text search.
//!
//! ## Example
//!
//! ```ignore
//! use ormdb_core::storage::stemmer::PorterStemmer;
//!
//! let stemmer = PorterStemmer::new();
//! assert_eq!(stemmer.stem("running"), "run");
//! assert_eq!(stemmer.stem("happiness"), "happi");
//! ```
//!
//! ## Algorithm
//!
//! The algorithm consists of 5 steps that progressively remove suffixes:
//! 1. Plurals and past participles (-s, -ed, -ing)
//! 2. Derivational suffixes (-ational, -fulness, etc.)
//! 3. More derivational suffixes (-icate, -ative, etc.)
//! 4. Residual suffixes (-ant, -ence, etc.)
//! 5. Final cleanup (-e, -ll)

/// Porter Stemmer for English text.
#[derive(Debug, Clone, Default)]
pub struct PorterStemmer;

impl PorterStemmer {
    /// Create a new Porter stemmer.
    pub fn new() -> Self {
        Self
    }

    /// Stem a word to its root form.
    pub fn stem(&self, word: &str) -> String {
        let word = word.to_lowercase();

        // Short words don't need stemming
        if word.len() <= 2 {
            return word;
        }

        let mut stem = word.clone();

        // Step 1a: Plurals
        stem = self.step1a(&stem);

        // Step 1b: Past tense and progressive
        stem = self.step1b(&stem);

        // Step 1c: Y to I
        stem = self.step1c(&stem);

        // Step 2: Derivational suffixes
        stem = self.step2(&stem);

        // Step 3: More derivational suffixes
        stem = self.step3(&stem);

        // Step 4: Residual suffixes
        stem = self.step4(&stem);

        // Step 5: Final cleanup
        stem = self.step5(&stem);

        stem
    }

    /// Check if a character is a consonant in the given word at position i.
    fn is_consonant(&self, word: &str, i: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if i >= chars.len() {
            return false;
        }

        match chars[i] {
            'a' | 'e' | 'i' | 'o' | 'u' => false,
            'y' => {
                if i == 0 {
                    true
                } else {
                    !self.is_consonant(word, i - 1)
                }
            }
            _ => true,
        }
    }

    /// Calculate the "measure" of a word (number of VC sequences).
    fn measure(&self, word: &str) -> usize {
        let chars: Vec<char> = word.chars().collect();
        if chars.is_empty() {
            return 0;
        }

        let mut m = 0;
        let mut i = 0;
        let n = chars.len();

        // Skip initial consonants
        while i < n && self.is_consonant(word, i) {
            i += 1;
        }

        while i < n {
            // Count vowel sequence
            while i < n && !self.is_consonant(word, i) {
                i += 1;
            }
            if i >= n {
                break;
            }

            // Count consonant sequence
            while i < n && self.is_consonant(word, i) {
                i += 1;
            }
            m += 1;
        }

        m
    }

    /// Check if word contains a vowel.
    fn contains_vowel(&self, word: &str) -> bool {
        for i in 0..word.len() {
            if !self.is_consonant(word, i) {
                return true;
            }
        }
        false
    }

    /// Check if word ends with a double consonant.
    fn ends_double_consonant(&self, word: &str) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() < 2 {
            return false;
        }
        let n = chars.len();
        chars[n - 1] == chars[n - 2] && self.is_consonant(word, n - 1)
    }

    /// Check if word ends with CVC where final C is not W, X, or Y.
    fn ends_cvc(&self, word: &str) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() < 3 {
            return false;
        }
        let n = chars.len();

        self.is_consonant(word, n - 3)
            && !self.is_consonant(word, n - 2)
            && self.is_consonant(word, n - 1)
            && !matches!(chars[n - 1], 'w' | 'x' | 'y')
    }

    /// Step 1a: Handle plurals.
    fn step1a(&self, word: &str) -> String {
        if word.ends_with("sses") {
            return word[..word.len() - 2].to_string();
        }
        if word.ends_with("ies") {
            return word[..word.len() - 2].to_string();
        }
        if word.ends_with("ss") {
            return word.to_string();
        }
        if word.ends_with('s') && word.len() > 1 {
            return word[..word.len() - 1].to_string();
        }
        word.to_string()
    }

    /// Step 1b: Handle past tense and progressive.
    fn step1b(&self, word: &str) -> String {
        if word.ends_with("eed") {
            let stem = &word[..word.len() - 3];
            if self.measure(stem) > 0 {
                return format!("{}ee", stem);
            }
            return word.to_string();
        }

        let (has_vowel_stem, stem) = if word.ends_with("ed") {
            let s = &word[..word.len() - 2];
            (self.contains_vowel(s), s.to_string())
        } else if word.ends_with("ing") {
            let s = &word[..word.len() - 3];
            (self.contains_vowel(s), s.to_string())
        } else {
            return word.to_string();
        };

        if !has_vowel_stem {
            return word.to_string();
        }

        // Apply additional rules
        if stem.ends_with("at") || stem.ends_with("bl") || stem.ends_with("iz") {
            return format!("{}e", stem);
        }

        if self.ends_double_consonant(&stem) {
            let chars: Vec<char> = stem.chars().collect();
            let last = chars[chars.len() - 1];
            if !matches!(last, 'l' | 's' | 'z') {
                return stem[..stem.len() - 1].to_string();
            }
        }

        if self.measure(&stem) == 1 && self.ends_cvc(&stem) {
            return format!("{}e", stem);
        }

        stem
    }

    /// Step 1c: Replace Y with I if there's a vowel in the stem.
    fn step1c(&self, word: &str) -> String {
        if word.ends_with('y') && word.len() > 1 {
            let stem = &word[..word.len() - 1];
            if self.contains_vowel(stem) {
                return format!("{}i", stem);
            }
        }
        word.to_string()
    }

    /// Step 2: Handle derivational suffixes.
    fn step2(&self, word: &str) -> String {
        let suffixes = [
            ("ational", "ate"),
            ("tional", "tion"),
            ("enci", "ence"),
            ("anci", "ance"),
            ("izer", "ize"),
            ("abli", "able"),
            ("alli", "al"),
            ("entli", "ent"),
            ("eli", "e"),
            ("ousli", "ous"),
            ("ization", "ize"),
            ("ation", "ate"),
            ("ator", "ate"),
            ("alism", "al"),
            ("iveness", "ive"),
            ("fulness", "ful"),
            ("ousness", "ous"),
            ("aliti", "al"),
            ("iviti", "ive"),
            ("biliti", "ble"),
        ];

        for (suffix, replacement) in suffixes {
            if word.ends_with(suffix) {
                let stem = &word[..word.len() - suffix.len()];
                if self.measure(stem) > 0 {
                    return format!("{}{}", stem, replacement);
                }
                return word.to_string();
            }
        }

        word.to_string()
    }

    /// Step 3: Handle more derivational suffixes.
    fn step3(&self, word: &str) -> String {
        let suffixes = [
            ("icate", "ic"),
            ("ative", ""),
            ("alize", "al"),
            ("iciti", "ic"),
            ("ical", "ic"),
            ("ful", ""),
            ("ness", ""),
        ];

        for (suffix, replacement) in suffixes {
            if word.ends_with(suffix) {
                let stem = &word[..word.len() - suffix.len()];
                if self.measure(stem) > 0 {
                    return format!("{}{}", stem, replacement);
                }
                return word.to_string();
            }
        }

        word.to_string()
    }

    /// Step 4: Handle residual suffixes.
    fn step4(&self, word: &str) -> String {
        let suffixes = [
            "al", "ance", "ence", "er", "ic", "able", "ible", "ant", "ement", "ment", "ent", "ion",
            "ou", "ism", "ate", "iti", "ous", "ive", "ize",
        ];

        for suffix in suffixes {
            if word.ends_with(suffix) {
                let stem = &word[..word.len() - suffix.len()];
                if self.measure(stem) > 1 {
                    // Special case for -ion: must be preceded by s or t
                    if suffix == "ion" {
                        let chars: Vec<char> = stem.chars().collect();
                        if !chars.is_empty() {
                            let last = chars[chars.len() - 1];
                            if last != 's' && last != 't' {
                                continue;
                            }
                        }
                    }
                    return stem.to_string();
                }
            }
        }

        word.to_string()
    }

    /// Step 5: Final cleanup.
    fn step5(&self, word: &str) -> String {
        let mut result = word.to_string();

        // Step 5a: Remove trailing e
        if result.ends_with('e') {
            let stem = &result[..result.len() - 1];
            let m = self.measure(stem);
            if m > 1 || (m == 1 && !self.ends_cvc(stem)) {
                result = stem.to_string();
            }
        }

        // Step 5b: Remove trailing double l if m > 1
        if result.ends_with("ll") && self.measure(&result) > 1 {
            result = result[..result.len() - 1].to_string();
        }

        result
    }
}

/// Tokenize text into words for indexing.
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty() && s.len() >= 2)
        .map(|s| s.to_string())
        .collect()
}

/// Tokenize and stem text for indexing.
pub fn tokenize_and_stem(text: &str) -> Vec<String> {
    let stemmer = PorterStemmer::new();
    tokenize(text)
        .into_iter()
        .map(|word| stemmer.stem(&word))
        .collect()
}

/// Common English stop words to exclude from indexing.
pub const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "he", "in", "is", "it",
    "its", "of", "on", "or", "that", "the", "to", "was", "were", "will", "with",
];

/// Check if a word is a stop word.
pub fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word.to_lowercase().as_str())
}

/// Tokenize, filter stop words, and stem text.
pub fn analyze(text: &str) -> Vec<String> {
    let stemmer = PorterStemmer::new();
    tokenize(text)
        .into_iter()
        .filter(|w| !is_stop_word(w))
        .map(|word| stemmer.stem(&word))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stem_plurals() {
        let stemmer = PorterStemmer::new();
        assert_eq!(stemmer.stem("cats"), "cat");
        assert_eq!(stemmer.stem("ponies"), "poni");
        assert_eq!(stemmer.stem("caresses"), "caress");
    }

    #[test]
    fn test_stem_past_tense() {
        let stemmer = PorterStemmer::new();
        assert_eq!(stemmer.stem("agreed"), "agre");
        assert_eq!(stemmer.stem("plastered"), "plaster");
        assert_eq!(stemmer.stem("bled"), "bled");
    }

    #[test]
    fn test_stem_progressive() {
        let stemmer = PorterStemmer::new();
        assert_eq!(stemmer.stem("running"), "run");
        assert_eq!(stemmer.stem("singing"), "sing");
        assert_eq!(stemmer.stem("conflating"), "conflat");
    }

    #[test]
    fn test_stem_derivational() {
        let stemmer = PorterStemmer::new();
        assert_eq!(stemmer.stem("relational"), "relat");
        assert_eq!(stemmer.stem("conditional"), "condit");
        assert_eq!(stemmer.stem("happiness"), "happi");
    }

    #[test]
    fn test_stem_common_words() {
        let stemmer = PorterStemmer::new();

        // These are standard Porter stemmer test cases
        assert_eq!(stemmer.stem("connect"), "connect");
        assert_eq!(stemmer.stem("connected"), "connect");
        assert_eq!(stemmer.stem("connecting"), "connect");
        assert_eq!(stemmer.stem("connection"), "connect");
        assert_eq!(stemmer.stem("connections"), "connect");
    }

    #[test]
    fn test_stem_edge_cases() {
        let stemmer = PorterStemmer::new();
        assert_eq!(stemmer.stem("a"), "a");
        assert_eq!(stemmer.stem(""), "");
        assert_eq!(stemmer.stem("the"), "the");
    }

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("Hello, World! This is a test.");
        assert_eq!(tokens, vec!["hello", "world", "this", "is", "test"]);
    }

    #[test]
    fn test_tokenize_and_stem() {
        let tokens = tokenize_and_stem("Running dogs are happy");
        assert_eq!(tokens, vec!["run", "dog", "ar", "happi"]);
    }

    #[test]
    fn test_analyze() {
        let tokens = analyze("The quick brown fox is jumping over the lazy dogs");
        // "the", "is", "the" are stop words and should be removed
        // remaining words are stemmed
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"brown".to_string()));
        assert!(tokens.contains(&"fox".to_string()));
        assert!(tokens.contains(&"jump".to_string()));
        assert!(tokens.contains(&"lazi".to_string()));
        assert!(tokens.contains(&"dog".to_string()));
        assert!(!tokens.contains(&"the".to_string()));
        assert!(!tokens.contains(&"is".to_string()));
    }

    #[test]
    fn test_stop_words() {
        assert!(is_stop_word("the"));
        assert!(is_stop_word("The"));
        assert!(is_stop_word("is"));
        assert!(!is_stop_word("cat"));
        assert!(!is_stop_word("running"));
    }
}
