use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const K1: f64 = 1.2;
const B: f64 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub filename: String,
    pub title: String,
    pub score: f64,
    pub snippet: String,
    pub url: String,
    pub start_line: usize,
    pub end_line: usize,
    pub matched_block: String,
    pub local_path: String,
}

struct Document {
    id: usize,
    filename: String,
    title: String,
    content: String,
    // term frequencies
    tf: HashMap<String, u32>,
    doc_len: usize,
    url: String,
    local_path: String,
}

pub struct SearchIndex {
    docs: Vec<Document>,
    df: HashMap<String, u32>, // document frequency
    avgdl: f64,
}

impl SearchIndex {
    pub fn new() -> Self {
        SearchIndex {
            docs: Vec::new(),
            df: HashMap::new(),
            avgdl: 0.0,
        }
    }

    pub fn add_document(
        &mut self,
        filename: String,
        title: String,
        content: String,
        url: String,
        local_path: String,
    ) {
        let tokens = tokenize(&content);
        let doc_len = tokens.len();

        let mut tf = HashMap::new();
        let mut unique_terms = HashSet::new();

        for token in tokens {
            *tf.entry(token.clone()).or_insert(0) += 1;
            unique_terms.insert(token);
        }

        for term in unique_terms {
            *self.df.entry(term).or_insert(0) += 1;
        }

        let doc = Document {
            id: self.docs.len(),
            filename,
            title,
            content,
            tf,
            doc_len,
            url,
            local_path,
        };

        self.docs.push(doc);
    }

    pub fn build(&mut self) {
        if self.docs.is_empty() {
            self.avgdl = 0.0;
        } else {
            let total_len: usize = self.docs.iter().map(|d| d.doc_len).sum();
            self.avgdl = total_len as f64 / self.docs.len() as f64;
        }
    }

    pub fn search(&self, query: &str, top_k: usize) -> Vec<SearchResult> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() || self.docs.is_empty() {
            return Vec::new();
        }

        let n = self.docs.len() as f64;
        let mut scores: Vec<(usize, f64)> = self
            .docs
            .iter()
            .enumerate()
            .map(|(i, doc)| {
                let mut score = 0.0;
                let mut matched_tokens = 0;

                for token in &query_tokens {
                    if let Some(&tf) = doc.tf.get(token) {
                        if let Some(&df) = self.df.get(token) {
                            matched_tokens += 1;
                            let idf = ((n - df as f64 + 0.5) / (df as f64 + 0.5) + 1.0).ln();
                            let tf_f = tf as f64;
                            let num = tf_f * (K1 + 1.0);
                            let den = tf_f + K1 * (1.0 - B + B * (doc.doc_len as f64 / self.avgdl));
                            score += idf * (num / den);
                        }
                    }
                }

                // Coordination Factor: significantly reward documents that match a higher overall proportion of the query.
                // This is critical for natural language sentences where short documents might otherwise
                // overscore by matching just 1 or 2 tokens due to BM25's length normalization.
                if !query_tokens.is_empty() && score > 0.0 {
                    let coverage = matched_tokens as f64 / query_tokens.len() as f64;
                    score = score * coverage.powf(1.5);
                }

                (i, score)
            })
            .filter(|&(_, score)| score > 0.0)
            .collect();

        // Sort descending by score
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scores
            .into_iter()
            .take(top_k)
            .map(|(id, score)| {
                let doc = &self.docs[id];
                let snippet = extract_snippet(&doc.content, &query_tokens);
                let (start_line, end_line, matched_block) =
                    extract_dense_block(&doc.content, &query_tokens);
                SearchResult {
                    filename: doc.filename.clone(),
                    title: doc.title.clone(),
                    score,
                    snippet,
                    url: doc.url.clone(),
                    start_line,
                    end_line,
                    matched_block,
                    local_path: doc.local_path.clone(),
                }
            })
            .collect()
    }
}

/// Tokenize string into bi-grams for CJK + words for latin
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let text = text.to_lowercase();

    // Quick heuristic: split by whitespace first
    let parts = text.split_whitespace();
    for part in parts {
        let part = part.trim_matches(|c: char| c.is_ascii_punctuation());
        if part.is_empty() {
            continue;
        }

        let chars: Vec<char> = part.chars().collect();
        let mut i = 0;
        let mut current_latin = String::new();

        while i < chars.len() {
            let c = chars[i];
            if is_cjk(c) {
                if !current_latin.is_empty() {
                    tokens.push(current_latin.clone());
                    current_latin.clear();
                }
                if i + 1 < chars.len() && is_cjk(chars[i + 1]) {
                    let mut bigram = String::new();
                    bigram.push(c);
                    bigram.push(chars[i + 1]);
                    tokens.push(bigram);
                } else {
                    // Single CJK char
                    tokens.push(c.to_string());
                }
                i += 1; // Slide window by 1 (overlapping bigrams)
            } else if c.is_alphanumeric() {
                current_latin.push(c);
                i += 1;
            } else {
                if !current_latin.is_empty() {
                    tokens.push(current_latin.clone());
                    current_latin.clear();
                }
                i += 1;
            }
        }
        if !current_latin.is_empty() {
            tokens.push(current_latin);
        }
    }
    tokens
}

fn is_cjk(c: char) -> bool {
    let u = c as u32;
    // CJK Unified Ideographs and common ranges
    (u >= 0x4E00 && u <= 0x9FFF) || (u >= 0x3400 && u <= 0x4DBF) || (u >= 0x20000 && u <= 0x2A6DF)
}

fn extract_snippet(content: &str, query_tokens: &[String]) -> String {
    // Very simple snippet extraction for display
    let content_lower = content.to_lowercase();

    // Find the first occurrence of any query token
    let mut best_idx = 0;
    for token in query_tokens {
        if let Some(idx) = content_lower.find(token) {
            best_idx = idx;
            break;
        }
    }

    let chars: Vec<char> = content.chars().collect();

    // Attempt to map byte index back to char index (approximate strategy)
    let char_idx = content[..best_idx].chars().count();

    let start = if char_idx > 30 { char_idx - 30 } else { 0 };
    let end = if start + 120 < chars.len() {
        start + 120
    } else {
        chars.len()
    };

    let mut snippet: String = chars[start..end].iter().collect();

    // Replace newlines with spaces
    snippet = snippet
        .replace('\n', " ")
        .replace('\r', "")
        .trim()
        .to_string();

    if start > 0 {
        snippet = format!("...{}", snippet);
    }
    if end < chars.len() {
        snippet.push_str("...");
    }

    snippet
}

fn extract_dense_block(content: &str, query_tokens: &[String]) -> (usize, usize, String) {
    let lines: Vec<&str> = content.lines().collect();
    let content_lower = content.to_lowercase();
    let lines_lower: Vec<&str> = content_lower.lines().collect();

    let mut line_hits: Vec<usize> = vec![0; lines.len()];
    let mut max_hits = 0;

    for (i, line) in lines_lower.iter().enumerate() {
        let mut hits = 0;
        for token in query_tokens {
            if line.contains(token) {
                hits += 1;
            }
        }
        if hits > max_hits {
            max_hits = hits;
        }
        line_hits[i] = hits;
    }

    if max_hits == 0 {
        return (0, 0, String::new());
    }

    let threshold = if max_hits >= 6 {
        std::cmp::max(2, max_hits / 4)
    } else {
        1
    };

    let mut hit_lines = Vec::new();
    for (i, &hits) in line_hits.iter().enumerate() {
        if hits >= threshold {
            hit_lines.push((i, hits));
        }
    }

    if hit_lines.is_empty() {
        return (0, 0, String::new());
    }

    let max_gap = 2;
    let mut best_start = hit_lines[0].0;
    let mut best_end = hit_lines[0].0;
    let mut best_score = 0;

    let mut curr_start = hit_lines[0].0;
    let mut curr_end = hit_lines[0].0;
    let mut curr_score = hit_lines[0].1;

    for i in 1..hit_lines.len() {
        let (line_idx, hits) = hit_lines[i];
        if line_idx - curr_end <= max_gap + 1 {
            curr_end = line_idx;
            curr_score += hits;
        } else {
            if curr_score > best_score {
                best_score = curr_score;
                best_start = curr_start;
                best_end = curr_end;
            }
            curr_start = line_idx;
            curr_end = line_idx;
            curr_score = hits;
        }
    }
    if curr_score > best_score {
        best_start = curr_start;
        best_end = curr_end;
    }

    let block_len = best_end.saturating_sub(best_start) + 1;
    if block_len > 15 {
        let mut peak_in_block = best_start;
        let mut local_max = 0;
        for j in best_start..=best_end {
            if line_hits[j] > local_max {
                local_max = line_hits[j];
                peak_in_block = j;
            }
        }
        best_start = peak_in_block.saturating_sub(5);
        best_end = std::cmp::min(peak_in_block + 10, best_end);
    }

    let block = lines[best_start..=best_end].join("\n");
    (best_start + 1, best_end + 1, block)
}
