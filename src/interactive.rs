use std::io::{self, BufRead, Write};

use dialoguer::{theme::ColorfulTheme, Select};

const SECTIONS: [(&str, &str); 7] = [
    ("1", "Broken links"),
    ("2", "Ambiguous links"),
    ("3", "Orphan notes"),
    ("4", "Stale notes"),
    ("5", "Hub notes"),
    ("6", "Clusters"),
    ("7", "Suggested links"),
];

pub fn run_interactive_report(report: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    writeln!(stdout, "rust-vault-map interactive scan")?;
    writeln!(stdout)?;
    write_section(report, "Summary", &mut stdout)?;
    writeln!(stdout)?;
    write_section(report, "Top findings", &mut stdout)?;
    writeln!(stdout)?;

    loop {
        let mut items = SECTIONS
            .iter()
            .map(|(_, label)| label.to_string())
            .collect::<Vec<_>>();
        items.push("Quit".to_string());

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(interactive_prompt())
            .items(&items)
            .default(0)
            .interact_opt()
            .map_err(io::Error::other)?;

        match selection {
            Some(index) if index < SECTIONS.len() => {
                writeln!(stdout)?;
                write_section(report, SECTIONS[index].1, &mut stdout)?;
                writeln!(stdout)?;
            }
            _ => break,
        }
    }

    Ok(())
}

pub fn interactive_prompt() -> &'static str {
    "Use arrow keys to choose a section"
}

pub fn run_interactive_report_with_io<R, W>(
    report: &str,
    mut reader: R,
    mut writer: W,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    writeln!(writer, "rust-vault-map interactive scan")?;
    writeln!(writer)?;
    write_section(report, "Summary", &mut writer)?;
    writeln!(writer)?;
    write_section(report, "Top findings", &mut writer)?;
    writeln!(writer)?;

    loop {
        writeln!(writer, "{}:", interactive_prompt())?;
        for (key, label) in SECTIONS {
            writeln!(writer, "{key}. {label}")?;
        }
        writeln!(writer, "q. Quit")?;
        write!(writer, "> ")?;
        writer.flush()?;

        let mut choice = String::new();
        if reader.read_line(&mut choice)? == 0 {
            break;
        }

        let choice = choice.trim();
        if choice.eq_ignore_ascii_case("q") {
            break;
        }

        match section_for_choice(choice) {
            Some(section) => {
                writeln!(writer)?;
                write_section(report, section, &mut writer)?;
                writeln!(writer)?;
            }
            None => {
                writeln!(writer, "Unknown section: {choice}")?;
                writeln!(writer)?;
            }
        }
    }

    Ok(())
}

fn section_for_choice(choice: &str) -> Option<&'static str> {
    SECTIONS
        .iter()
        .find_map(|(key, label)| (*key == choice).then_some(*label))
}

fn write_section<W: Write>(report: &str, section: &str, writer: &mut W) -> io::Result<()> {
    writeln!(writer, "{section}")?;
    for line in section_lines(report, section) {
        writeln!(writer, "{line}")?;
    }
    Ok(())
}

fn section_lines<'a>(report: &'a str, section: &str) -> Vec<&'a str> {
    let headings = [
        "Summary",
        "Top findings",
        "Broken links",
        "Ambiguous links",
        "Orphan notes",
        "Stale notes",
        "Hub notes",
        "Clusters",
        "Suggested links",
        "Next actions",
    ];

    let mut in_section = false;
    let mut lines = Vec::new();

    for line in report.lines() {
        let plain_line = strip_ansi(line);
        if plain_line == section {
            in_section = true;
            continue;
        }

        if in_section && headings.contains(&plain_line.as_str()) {
            break;
        }

        if in_section {
            lines.push(line);
        }
    }

    while matches!(lines.last(), Some(line) if line.is_empty()) {
        lines.pop();
    }

    lines
}

fn strip_ansi(line: &str) -> String {
    let mut plain = String::new();
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            plain.push(ch);
        }
    }

    plain
}
