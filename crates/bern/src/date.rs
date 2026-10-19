//! Calendar dates for protocols and declarations.
//!
//! `CalendarDate` is the framework type of a day on the calendar: the
//! engine gives the date, and the interface writes it in the reader's
//! language with `{name, date, pattern}` in `tr!`, such as
//! `"{today, date, EEE, MMM d}"` for "Mon, Oct 12". It has no time of day
//! and no time zone.

use crate::bind::{BernValue, Binding, Readable, ToRef};
use bern_protocol::{SchemaError, Value};

/// A day on the calendar: year, month 1–12, day 1–31.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CalendarDate {
    pub year: i64,
    pub month: i64,
    pub day: i64,
}

impl CalendarDate {
    pub fn new(year: i64, month: i64, day: i64) -> Self {
        Self { year, month, day }
    }

    /// The day of the week, 0 for Monday to 6 for Sunday.
    pub fn weekday(&self) -> usize {
        bern_protocol::format::weekday(self.year, self.month, self.day)
    }
}

/// Typed reference to a `CalendarDate` inside declarations.
#[derive(Clone)]
pub struct CalendarDateRef {
    pub year: Binding<i64>,
    pub month: Binding<i64>,
    pub day: Binding<i64>,
}

impl BernValue for CalendarDate {
    type Ref = CalendarDateRef;
    fn reference() -> Self::Ref {
        CalendarDateRef {
            year: Binding::declared(),
            month: Binding::declared(),
            day: Binding::declared(),
        }
    }
    fn to_value(&self) -> Value {
        Value::Record(vec![
            Value::Int(self.year),
            Value::Int(self.month),
            Value::Int(self.day),
        ])
    }
    fn from_value(value: &Value) -> Result<Self, SchemaError> {
        match value {
            Value::Record(fields) if fields.len() == 3 => Ok(CalendarDate {
                year: i64::from_value(&fields[0])?,
                month: i64::from_value(&fields[1])?,
                day: i64::from_value(&fields[2])?,
            }),
            _ => Err(SchemaError::new("expected a `CalendarDate` record")),
        }
    }
}

impl Readable<CalendarDate> for CalendarDateRef {}

impl ToRef for CalendarDateRef {
    type Ref = CalendarDateRef;
    fn to_ref(&self) -> CalendarDateRef {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_date_matches_its_schema() {
        let date = CalendarDate::new(2026, 10, 12);
        assert_eq!(date.weekday(), 0);
        let mut types = bern_protocol::TypeTable::default();
        types
            .records
            .push(bern_protocol::format::calendar_date_def());
        date.to_value()
            .check(
                &bern_protocol::Type::Record {
                    name: bern_protocol::format::CALENDAR_DATE.into(),
                },
                &types,
            )
            .unwrap();
        assert_eq!(CalendarDate::from_value(&date.to_value()).unwrap(), date);
    }
}
