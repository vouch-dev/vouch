use super::report;
use anyhow::Result;
use prettytable::{self, cell};

/// Generates and returns a table from a given extension dependancy review report.
pub fn get(dependancy_reports: &Vec<report::DependancyReport>) -> Result<prettytable::Table> {
    let mut table = prettytable::Table::new();
    table.set_titles(prettytable::row![c => "  ", "name", "version", "reviews", "notes"]);
    table.set_format(*prettytable::format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);

    let mut dependancy_reports = dependancy_reports.clone();
    dependancy_reports.sort();

    for dependancy in dependancy_reports {
        let status_call: prettytable::Cell = dependancy.status.clone().into();
        let package_version = match &dependancy.version {
            Some(v) => v.as_str(),
            None => "",
        };
        let review_count = match dependancy.review_count {
            Some(v) => v.to_string(),
            None => "".to_string(),
        };
        let note = get_note_cell(&dependancy);

        table.add_row(prettytable::Row::new(vec![
            status_call,
            prettytable::Cell::new_align(&dependancy.name, prettytable::format::Alignment::LEFT),
            prettytable::Cell::new_align(&package_version, prettytable::format::Alignment::RIGHT),
            prettytable::Cell::new_align(&review_count, prettytable::format::Alignment::RIGHT),
            note,
        ]));
    }
    Ok(table)
}

fn get_note_cell(dependancy_report: &report::DependancyReport) -> prettytable::Cell {
    let note = match &dependancy_report.note {
        Some(v) => v.as_str(),
        None => "",
    };
    let mut note = prettytable::Cell::new_align(&note, prettytable::format::Alignment::LEFT);

    if dependancy_report.status == report::DependancyStatus::Fail {
        note = note
            .with_style(prettytable::Attr::BackgroundColor(
                prettytable::color::BRIGHT_RED,
            ))
            .with_style(prettytable::Attr::ForegroundColor(
                prettytable::color::BLACK,
            ));
    }
    note
}

impl From<report::DependancyStatus> for prettytable::Cell {
    fn from(report: report::DependancyStatus) -> Self {
        let lebel = match report {
            report::DependancyStatus::Pass => " PASS ",
            report::DependancyStatus::Warn => " WARN ",
            report::DependancyStatus::Fail => " FAIL ",
        };

        let background_color = match report {
            report::DependancyStatus::Pass => prettytable::color::BRIGHT_GREEN,
            report::DependancyStatus::Warn => prettytable::color::YELLOW,
            report::DependancyStatus::Fail => prettytable::color::BRIGHT_RED,
        };

        prettytable::Cell::new_align(lebel, prettytable::format::Alignment::CENTER)
            .with_style(prettytable::Attr::BackgroundColor(background_color))
            .with_style(prettytable::Attr::ForegroundColor(
                prettytable::color::BLACK,
            ))
    }
}
