//! Text with positions from the ASD-STE100 PDF.
//!
//! `pdf-extract` reports each glyph with its text matrix. We collect the
//! glyphs into words, the words into lines (by baseline), and the lines into
//! pages. We keep the x-coordinates. Thus the columns of the dictionary table
//! come from the geometry and not from character offsets.

use anyhow::{Context, Result};
use pdf_extract::{Document, MediaBox, OutputDev, OutputError, Transform, output_doc};

#[derive(Debug, Clone)]
pub struct Word {
    pub x0: f64,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Line {
    pub y: f64,
    pub words: Vec<Word>,
}

impl Line {
    pub fn text(&self) -> String {
        join(&self.words)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Page {
    pub number: u32,
    pub lines: Vec<Line>,
}

/// Join the words with one space between them.
pub fn join(words: &[Word]) -> String {
    let mut s = String::new();
    for w in words {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(&w.text);
    }
    s
}

#[derive(Debug, Clone)]
struct Glyph {
    x: f64,
    y: f64,
    advance: f64,
    size: f64,
    text: String,
}

#[derive(Default)]
struct Collector {
    page_height: f64,
    page_number: u32,
    glyphs: Vec<Glyph>,
    pages: Vec<Page>,
}

impl Collector {
    /// Group the glyphs of this page into lines and words.
    fn finish_page(&mut self) {
        let mut glyphs = std::mem::take(&mut self.glyphs);
        if glyphs.is_empty() {
            return;
        }
        // Sort by baseline (top to bottom), then x.
        glyphs.sort_by(|a, b| {
            a.y.partial_cmp(&b.y)
                .unwrap()
                .then(a.x.partial_cmp(&b.x).unwrap())
        });

        let mut lines: Vec<Line> = Vec::new();
        let mut current: Vec<Glyph> = Vec::new();
        let mut current_y = f64::NAN;
        for g in glyphs {
            let tol = g.size * 0.4;
            if current.is_empty() || (g.y - current_y).abs() <= tol {
                if current.is_empty() {
                    current_y = g.y;
                }
                current.push(g);
            } else {
                lines.push(words_from(current_y, std::mem::take(&mut current)));
                current_y = g.y;
                current.push(g);
            }
        }
        if !current.is_empty() {
            lines.push(words_from(current_y, current));
        }
        lines.retain(|l| !l.words.is_empty());
        self.pages.push(Page {
            number: self.page_number,
            lines,
        });
    }
}

/// Split the glyphs of one line into words at whitespace or at horizontal
/// spaces.
fn words_from(y: f64, mut glyphs: Vec<Glyph>) -> Line {
    glyphs.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    let mut words: Vec<Word> = Vec::new();
    let mut text = String::new();
    let mut x0 = 0.0;
    let mut last_end = f64::NEG_INFINITY;
    let mut last_size: f64 = 0.0;

    let flush = |text: &mut String, words: &mut Vec<Word>, x0: f64| {
        let t = text.trim();
        if !t.is_empty() {
            words.push(Word {
                x0,
                text: t.to_string(),
            });
        }
        text.clear();
    };

    for g in glyphs {
        let is_space = g.text.chars().all(char::is_whitespace);
        let gap = g.x - last_end;
        let new_word = text.is_empty() || gap > last_size.max(g.size) * 0.22;
        if is_space {
            flush(&mut text, &mut words, x0);
            last_end = g.x + g.advance;
            last_size = g.size;
            continue;
        }
        if new_word {
            flush(&mut text, &mut words, x0);
            x0 = g.x;
        }
        text.push_str(&g.text);
        last_end = g.x + g.advance;
        last_size = g.size;
    }
    flush(&mut text, &mut words, x0);
    Line { y, words }
}

impl OutputDev for Collector {
    fn begin_page(
        &mut self,
        page_num: u32,
        media_box: &MediaBox,
        _: Option<(f64, f64, f64, f64)>,
    ) -> std::result::Result<(), OutputError> {
        self.page_number = page_num;
        self.page_height = media_box.ury - media_box.lly;
        Ok(())
    }

    fn end_page(&mut self) -> std::result::Result<(), OutputError> {
        self.finish_page();
        Ok(())
    }

    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        _spacing: f64,
        font_size: f64,
        text: &str,
    ) -> std::result::Result<(), OutputError> {
        // The font size after the text matrix, calculated as `PlainTextOutput`
        // does: the geometric mean of the x scale and the y scale.
        let sx = (trm.m11 * trm.m11 + trm.m12 * trm.m12).sqrt();
        let sy = (trm.m21 * trm.m21 + trm.m22 * trm.m22).sqrt();
        let size = font_size * (sx * sy).sqrt();
        self.glyphs.push(Glyph {
            x: trm.m31,
            // Make y increase down the page.
            y: self.page_height - trm.m32,
            advance: width * size,
            size,
            text: text.to_string(),
        });
        Ok(())
    }

    fn begin_word(&mut self) -> std::result::Result<(), OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> std::result::Result<(), OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> std::result::Result<(), OutputError> {
        Ok(())
    }
}

/// Load a PDF and give its pages as lines of words with positions.
pub fn extract_pages(bytes: &[u8]) -> Result<Vec<Page>> {
    let doc = Document::load_mem(bytes).context("failed to parse PDF")?;
    let mut collector = Collector::default();
    output_doc(&doc, &mut collector).context("failed to extract text from PDF")?;
    Ok(collector.pages)
}
