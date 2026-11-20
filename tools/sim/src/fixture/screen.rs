// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Recorded console screens: a text file of rows, their colours and
//! their flags (`screens/*.screen`), read into a `qsipart` frame.

use qsipart::console::{Cell, Frame, Shape, Status, attr};

/// Work with Active Jobs, the prototype's terminal picture.
pub const WRKACTJOB: &str = include_str!("../../screens/wrkactjob.screen");

/// A recorded screen: the frame and the name the status row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorded {
    /// The frame a console's attach sends first.
    pub frame: Frame,
    /// The system's name, as its status row shows it.
    pub system: String,
}

fn color(c: char) -> Option<u16> {
    let base = match c.to_ascii_lowercase() {
        'g' => attr::GREEN,
        'w' => attr::WHITE,
        'r' => attr::RED,
        'b' => attr::BLUE,
        't' => attr::TURQUOISE,
        'p' => attr::PINK,
        'y' => attr::YELLOW,
        _ => return None,
    };
    Some(if c.is_ascii_uppercase() {
        base | attr::HIGH
    } else {
        base
    })
}

fn flags(c: char) -> Option<u16> {
    Some(match c {
        'u' => attr::UNDERLINE,
        'r' => attr::REVERSE,
        'n' => attr::NONDISPLAY,
        'c' => attr::COLUMN_SEP,
        '.' => 0,
        _ => return None,
    })
}

/// Read a recorded screen.
///
/// # Errors
/// A header, a row or a cell that does not read, as words.
pub fn read(text: &str) -> Result<Recorded, String> {
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let (mut rows, mut cols, mut cursor, mut system) = (0u8, 0u8, (0u8, 0u8), String::new());
    for l in lines.by_ref() {
        if l.is_empty() {
            break;
        }
        let words: Vec<&str> = l.split_whitespace().collect();
        let num = |i: usize| words.get(i).and_then(|w| w.parse::<u8>().ok());
        match words.first() {
            Some(&"size") => (rows, cols) = (num(1).unwrap_or(0), num(2).unwrap_or(0)),
            Some(&"cursor") => cursor = (num(1).unwrap_or(0), num(2).unwrap_or(0)),
            Some(&"system") => system = words.get(1).unwrap_or(&"").to_string(),
            _ => return Err(format!("a header line that is not one: {l}")),
        }
    }
    let (r, c) = (usize::from(rows), usize::from(cols));
    let block = |lines: &mut dyn Iterator<Item = &str>| -> Result<Vec<Vec<char>>, String> {
        let b: Vec<Vec<char>> = lines.take(r).map(|l| l.chars().collect()).collect();
        if b.len() != r || b.iter().any(|l| l.len() != c) {
            return Err(format!("a block that is not {r} rows of {c}"));
        }
        let _ = lines.next();
        Ok(b)
    };
    let text = block(&mut lines)?;
    let colors = block(&mut lines)?;
    let marks = block(&mut lines)?;
    let mut cells = Vec::with_capacity(r * c);
    for y in 0..r {
        for x in 0..c {
            let a = color(colors[y][x]).ok_or_else(|| format!("a colour at {y},{x}"))?
                | flags(marks[y][x]).ok_or_else(|| format!("a flag at {y},{x}"))?;
            cells.push(Cell {
                ch: text[y][x],
                attr: a,
            });
        }
    }
    let frame = Frame {
        seq: 1,
        rows,
        cols,
        cells,
        status: Status {
            row: cursor.0,
            col: cursor.1,
            shape: Shape::Underline,
            indicators: 0,
            src: None,
        },
    };
    frame.check().map_err(|e| e.to_string())?;
    Ok(Recorded { frame, system })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The recorded screen drawn by QS/400's look is the prototype's
    /// picture, byte for byte.
    #[test]
    fn the_recorded_screen_draws_the_prototype_picture() {
        let r = read(WRKACTJOB).unwrap();
        let mut grid = qsipart::console::Grid::default();
        grid.apply(&qsipart::console::FromSystem::Frame(r.frame))
            .unwrap();
        let view = termlook::screen::grid::View {
            grid,
            system: r.system,
            connected: true,
            tls: true,
            scheme: termlook::look::render::Scheme::default(),
        };
        let golden = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../design/qsmc-prototype/shared/img/5250-24x80.png"),
        )
        .unwrap();
        assert!(
            view.png(1) == golden,
            "the picture differs from the prototype's"
        );
    }
}
