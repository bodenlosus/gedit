use pyo3::{exceptions::PyOSError, prelude::*};
use std::error::Error;
use std::sync::RwLock;
use tree_sitter_highlight as hl;

#[pyclass]
pub enum HLEvent {
    Source(usize, usize),
    Start(String),
    End(),
}

#[pyclass]
pub struct Highlighter {
    highlighter: RwLock<hl::Highlighter>,
    recognized_names: Vec<String>,
    configuration: RwLock<Option<hl::HighlightConfiguration>>,
}
#[derive(Debug)]
struct HighlighterError {
    message: String,
}

impl HighlighterError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl Error for HighlighterError {}

impl std::fmt::Display for HighlighterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error doing Syntax Highlighting {}", self.message)
    }
}

// impl std::convert::From<HighlighterError> for PyErr {
//     fn from(value: HighlighterError) -> Self {
//         PyOSError::new_err(value.to_string())
//     }
// }

impl std::convert::Into<PyErr> for HighlighterError {
    fn into(self) -> PyErr {
        PyOSError::new_err(self.to_string())
    }
}
#[pymethods]
impl Highlighter {
    #[new]
    pub fn new(recognized_names: Vec<String>) -> Self {
        Self {
            highlighter: RwLock::new(hl::Highlighter::new()),
            recognized_names,
            configuration: RwLock::new(None),
        }
    }
    pub fn set_language(&self) -> PyResult<()> {
        let lang = tree_sitter_python::LANGUAGE;
        let mut config = match hl::HighlightConfiguration::new(
            lang.into(),
            "python",
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
            tree_sitter_python::TAGS_QUERY,
        ) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("{e}");
                return Err(
                    HighlighterError::new("Could not create Highlighter Function".into()).into(),
                );
            }
        };

        config.configure(&self.recognized_names);

        let Ok(mut c) = self.configuration.try_write() else {
            return Err(HighlighterError::new("Could not lock on config".into()).into());
        };
        *c = Some(config);

        Ok(())
    }
    pub fn highlight(&self, string: &str) -> PyResult<Vec<HLEvent>> {
        let leading_white_space = string
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(string.len());

        let string = string.trim_start().trim_end();
        let config = self.configuration.try_read();
        let Ok(config) = config.as_ref() else {
            return Err(HighlighterError::new("Could not lock on config".into()).into());
        };

        let Some(config) = config.as_ref() else {
            return Err(HighlighterError::new("Could not lock on config".into()).into());
        };

        let Ok(mut highligher) = self.highlighter.try_write() else {
            return Err(HighlighterError::new("Could not lock on highlighter".into()).into());
        };

        let highlights = match highligher.highlight(config, string.as_bytes(), None, |_| None) {
            Ok(hl) => hl,
            Err(e) => {
                eprintln!("{e}");
                return Err(HighlighterError::new("Error while highlighting".into()).into());
            }
        };

        let mut res: Vec<HLEvent> = Vec::new();

        for event in highlights {
            let Ok(event) = event else {
                continue;
            };

            match event {
                tree_sitter_highlight::HighlightEvent::Source { start, end } => res.push(
                    HLEvent::Source(start + leading_white_space, end + leading_white_space),
                ),
                tree_sitter_highlight::HighlightEvent::HighlightStart(highlight) => {
                    let hl_type = self.recognized_names.get(highlight.0);
                    if let Some(hl_type) = hl_type {
                        res.push(HLEvent::Start(hl_type.clone()));
                    }
                }
                tree_sitter_highlight::HighlightEvent::HighlightEnd => res.push(HLEvent::End()),
            }
        }

        Ok(res)
    }
}
