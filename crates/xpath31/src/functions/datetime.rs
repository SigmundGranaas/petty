use crate::error::XPath31Error;
use crate::types::{AtomicValue, XdmValue};
use regex::Regex;
use std::fmt;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// xs:dateTime
#[derive(Debug, Clone, PartialEq)]
pub struct DateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: f64,
    pub timezone: Option<Timezone>,
}

/// xs:date
#[derive(Debug, Clone, PartialEq)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub timezone: Option<Timezone>,
}

/// xs:time
#[derive(Debug, Clone, PartialEq)]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
    pub second: f64,
    pub timezone: Option<Timezone>,
}

/// xs:duration
#[derive(Debug, Clone, PartialEq)]
pub struct Duration {
    pub negative: bool,
    pub years: i32,
    pub months: i32,
    pub days: i32,
    pub hours: i32,
    pub minutes: i32,
    pub seconds: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Timezone {
    pub offset_minutes: i32,
}

static DATETIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(-?\d{4,})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2}(?:\.\d+)?)(Z|[+-]\d{2}:\d{2})?$",
    )
    .expect("BUG: invalid DATETIME_RE regex literal")
});

static DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(-?\d{4,})-(\d{2})-(\d{2})(Z|[+-]\d{2}:\d{2})?$")
        .expect("BUG: invalid DATE_RE regex literal")
});

static TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{2}):(\d{2}):(\d{2}(?:\.\d+)?)(Z|[+-]\d{2}:\d{2})?$")
        .expect("BUG: invalid TIME_RE regex literal")
});

static DURATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(-)?P(?:(\d+)Y)?(?:(\d+)M)?(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:(\d+(?:\.\d+)?)S)?)?$",
    )
    .expect("BUG: invalid DURATION_RE regex literal")
});

static DAY_TIME_DURATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(-)?P(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:(\d+(?:\.\d+)?)S)?)?$")
        .expect("BUG: invalid DAY_TIME_DURATION_RE regex literal")
});

impl DateTime {
    pub fn parse(s: &str) -> Option<Self> {
        let caps = DATETIME_RE.captures(s.trim())?;

        let year: i32 = caps.get(1)?.as_str().parse().ok()?;
        let month: u8 = caps.get(2)?.as_str().parse().ok()?;
        let day: u8 = caps.get(3)?.as_str().parse().ok()?;
        let hour: u8 = caps.get(4)?.as_str().parse().ok()?;
        let minute: u8 = caps.get(5)?.as_str().parse().ok()?;
        let second: f64 = caps.get(6)?.as_str().parse().ok()?;
        let timezone = caps.get(7).and_then(|m| Timezone::parse(m.as_str()));

        if !(1..=12).contains(&month)
            || !(1..=31).contains(&day)
            || hour > 24
            || minute > 59
            || second >= 60.0
        {
            return None;
        }

        Some(DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            timezone,
        })
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tz = self
            .timezone
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_default();

        if self.second.fract() == 0.0 {
            write!(
                f,
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}",
                self.year, self.month, self.day, self.hour, self.minute, self.second as i32, tz
            )
        } else {
            // Format with leading zero for single-digit seconds (e.g., 05.123456 not 5.123456)
            // Width 09 = 2 digits + 1 decimal + 6 fractional digits
            write!(
                f,
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:09.6}{}",
                self.year, self.month, self.day, self.hour, self.minute, self.second, tz
            )
        }
    }
}

impl Date {
    pub fn parse(s: &str) -> Option<Self> {
        let caps = DATE_RE.captures(s.trim())?;

        let year: i32 = caps.get(1)?.as_str().parse().ok()?;
        let month: u8 = caps.get(2)?.as_str().parse().ok()?;
        let day: u8 = caps.get(3)?.as_str().parse().ok()?;
        let timezone = caps.get(4).and_then(|m| Timezone::parse(m.as_str()));

        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }

        Some(Date {
            year,
            month,
            day,
            timezone,
        })
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tz = self
            .timezone
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_default();
        write!(
            f,
            "{:04}-{:02}-{:02}{}",
            self.year, self.month, self.day, tz
        )
    }
}

impl Time {
    pub fn parse(s: &str) -> Option<Self> {
        let caps = TIME_RE.captures(s.trim())?;

        let hour: u8 = caps.get(1)?.as_str().parse().ok()?;
        let minute: u8 = caps.get(2)?.as_str().parse().ok()?;
        let second: f64 = caps.get(3)?.as_str().parse().ok()?;
        let timezone = caps.get(4).and_then(|m| Timezone::parse(m.as_str()));

        if hour > 24 || minute > 59 || second >= 60.0 {
            return None;
        }

        Some(Time {
            hour,
            minute,
            second,
            timezone,
        })
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tz = self
            .timezone
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_default();

        if self.second.fract() == 0.0 {
            write!(
                f,
                "{:02}:{:02}:{:02}{}",
                self.hour, self.minute, self.second as i32, tz
            )
        } else {
            // Format with leading zero for single-digit seconds (e.g., 05.123456 not 5.123456)
            // Width 09 = 2 digits + 1 decimal + 6 fractional digits
            write!(
                f,
                "{:02}:{:02}:{:09.6}{}",
                self.hour, self.minute, self.second, tz
            )
        }
    }
}

impl Duration {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let caps = DURATION_RE.captures(s)?;

        let negative = caps.get(1).is_some();
        let years: i32 = caps
            .get(2)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);
        let months: i32 = caps
            .get(3)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);
        let days: i32 = caps
            .get(4)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);
        let hours: i32 = caps
            .get(5)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);
        let minutes: i32 = caps
            .get(6)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);
        let seconds: f64 = caps
            .get(7)
            .map(|m| m.as_str().parse().unwrap_or(0.0))
            .unwrap_or(0.0);

        Some(Duration {
            negative,
            years,
            months,
            days,
            hours,
            minutes,
            seconds,
        })
    }

    pub fn parse_day_time(s: &str) -> Option<Self> {
        let s = s.trim();
        let caps = DAY_TIME_DURATION_RE.captures(s)?;

        let negative = caps.get(1).is_some();
        let days: i32 = caps
            .get(2)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);
        let hours: i32 = caps
            .get(3)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);
        let minutes: i32 = caps
            .get(4)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);
        let seconds: f64 = caps
            .get(5)
            .map(|m| m.as_str().parse().unwrap_or(0.0))
            .unwrap_or(0.0);

        Some(Duration {
            negative,
            years: 0,
            months: 0,
            days,
            hours,
            minutes,
            seconds,
        })
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut result = String::new();
        if self.negative {
            result.push('-');
        }
        result.push('P');

        if self.years != 0 {
            result.push_str(&format!("{}Y", self.years));
        }
        if self.months != 0 {
            result.push_str(&format!("{}M", self.months));
        }
        if self.days != 0 {
            result.push_str(&format!("{}D", self.days));
        }

        if self.hours != 0 || self.minutes != 0 || self.seconds != 0.0 {
            result.push('T');
            if self.hours != 0 {
                result.push_str(&format!("{}H", self.hours));
            }
            if self.minutes != 0 {
                result.push_str(&format!("{}M", self.minutes));
            }
            if self.seconds != 0.0 {
                if self.seconds.fract() == 0.0 {
                    result.push_str(&format!("{}S", self.seconds as i32));
                } else {
                    result.push_str(&format!("{}S", self.seconds));
                }
            }
        }

        if result == "P" || result == "-P" {
            return write!(f, "PT0S");
        }

        write!(f, "{}", result)
    }
}

impl Timezone {
    pub fn parse(s: &str) -> Option<Self> {
        if s == "Z" {
            return Some(Timezone { offset_minutes: 0 });
        }

        if s.len() != 6 {
            return None;
        }

        let sign = match s.chars().next()? {
            '+' => 1,
            '-' => -1,
            _ => return None,
        };

        let hours: i32 = s[1..3].parse().ok()?;
        let minutes: i32 = s[4..6].parse().ok()?;

        if hours > 14 || minutes > 59 {
            return None;
        }

        Some(Timezone {
            offset_minutes: sign * (hours * 60 + minutes),
        })
    }
}

impl fmt::Display for Timezone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.offset_minutes == 0 {
            write!(f, "Z")
        } else {
            let sign = if self.offset_minutes >= 0 { '+' } else { '-' };
            let abs_minutes = self.offset_minutes.abs();
            let hours = abs_minutes / 60;
            let mins = abs_minutes % 60;
            write!(f, "{}{:02}:{:02}", sign, hours, mins)
        }
    }
}

fn get_current_utc_datetime() -> DateTime {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let total_secs = now.as_secs();
    let nanos = now.subsec_nanos();

    let days_since_epoch = (total_secs / 86400) as i64;
    let secs_in_day = (total_secs % 86400) as u32;

    let hour = (secs_in_day / 3600) as u8;
    let minute = ((secs_in_day % 3600) / 60) as u8;
    let second = (secs_in_day % 60) as f64 + (nanos as f64 / 1_000_000_000.0);

    let (year, month, day) = days_to_ymd(days_since_epoch + 719_468);

    DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
        timezone: Some(Timezone { offset_minutes: 0 }),
    }
}

fn days_to_ymd(days: i64) -> (i32, u8, u8) {
    let era = if days >= 0 {
        days / 146097
    } else {
        (days - 146096) / 146097
    };
    let doe = (days - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    (year as i32, m as u8, d as u8)
}

pub fn fn_current_datetime<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if !args.is_empty() {
        return Err(XPath31Error::function(
            "current-dateTime",
            "expects no arguments",
        ));
    }
    let dt = get_current_utc_datetime();
    Ok(XdmValue::from_atomic(AtomicValue::DateTime(dt.to_string())))
}

pub fn fn_current_date<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if !args.is_empty() {
        return Err(XPath31Error::function(
            "current-date",
            "expects no arguments",
        ));
    }
    let dt = get_current_utc_datetime();
    let date = Date {
        year: dt.year,
        month: dt.month,
        day: dt.day,
        timezone: Some(Timezone { offset_minutes: 0 }),
    };
    Ok(XdmValue::from_atomic(AtomicValue::Date(date.to_string())))
}

pub fn fn_current_time<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if !args.is_empty() {
        return Err(XPath31Error::function(
            "current-time",
            "expects no arguments",
        ));
    }
    let dt = get_current_utc_datetime();
    let time = Time {
        hour: dt.hour,
        minute: dt.minute,
        second: dt.second,
        timezone: Some(Timezone { offset_minutes: 0 }),
    };
    Ok(XdmValue::from_atomic(AtomicValue::Time(time.to_string())))
}

pub fn fn_datetime<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "dateTime",
            "expects exactly 2 arguments",
        ));
    }

    let date_str = args[0].to_string_value();
    let time_str = args[1].to_string_value();

    let date = Date::parse(&date_str)
        .ok_or_else(|| XPath31Error::function("dateTime", format!("Invalid date: {}", date_str)))?;

    let time = Time::parse(&time_str)
        .ok_or_else(|| XPath31Error::function("dateTime", format!("Invalid time: {}", time_str)))?;

    let tz = match (&date.timezone, &time.timezone) {
        (Some(d), Some(t)) if d != t => {
            return Err(XPath31Error::function(
                "dateTime",
                "Date and time have incompatible timezones",
            ));
        }
        (Some(d), _) => Some(d.clone()),
        (_, Some(t)) => Some(t.clone()),
        _ => None,
    };

    let dt = DateTime {
        year: date.year,
        month: date.month,
        day: date.day,
        hour: time.hour,
        minute: time.minute,
        second: time.second,
        timezone: tz,
    };

    Ok(XdmValue::from_atomic(AtomicValue::DateTime(dt.to_string())))
}

pub fn fn_year_from_datetime<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "year-from-dateTime",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let dt_str = args[0].to_string_value();
    let dt = DateTime::parse(&dt_str).ok_or_else(|| {
        XPath31Error::function(
            "year-from-dateTime",
            format!("Invalid dateTime: {}", dt_str),
        )
    })?;
    Ok(XdmValue::from_integer(dt.year as i64))
}

pub fn fn_month_from_datetime<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "month-from-dateTime",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let dt_str = args[0].to_string_value();
    let dt = DateTime::parse(&dt_str).ok_or_else(|| {
        XPath31Error::function(
            "month-from-dateTime",
            format!("Invalid dateTime: {}", dt_str),
        )
    })?;
    Ok(XdmValue::from_integer(dt.month as i64))
}

pub fn fn_day_from_datetime<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "day-from-dateTime",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let dt_str = args[0].to_string_value();
    let dt = DateTime::parse(&dt_str).ok_or_else(|| {
        XPath31Error::function("day-from-dateTime", format!("Invalid dateTime: {}", dt_str))
    })?;
    Ok(XdmValue::from_integer(dt.day as i64))
}

pub fn fn_hours_from_datetime<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "hours-from-dateTime",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let dt_str = args[0].to_string_value();
    let dt = DateTime::parse(&dt_str).ok_or_else(|| {
        XPath31Error::function(
            "hours-from-dateTime",
            format!("Invalid dateTime: {}", dt_str),
        )
    })?;
    Ok(XdmValue::from_integer(dt.hour as i64))
}

pub fn fn_minutes_from_datetime<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "minutes-from-dateTime",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let dt_str = args[0].to_string_value();
    let dt = DateTime::parse(&dt_str).ok_or_else(|| {
        XPath31Error::function(
            "minutes-from-dateTime",
            format!("Invalid dateTime: {}", dt_str),
        )
    })?;
    Ok(XdmValue::from_integer(dt.minute as i64))
}

pub fn fn_seconds_from_datetime<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "seconds-from-dateTime",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let dt_str = args[0].to_string_value();
    let dt = DateTime::parse(&dt_str).ok_or_else(|| {
        XPath31Error::function(
            "seconds-from-dateTime",
            format!("Invalid dateTime: {}", dt_str),
        )
    })?;
    Ok(XdmValue::from_double(dt.second))
}

pub fn fn_timezone_from_datetime<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "timezone-from-dateTime",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let dt_str = args[0].to_string_value();
    let dt = DateTime::parse(&dt_str).ok_or_else(|| {
        XPath31Error::function(
            "timezone-from-dateTime",
            format!("Invalid dateTime: {}", dt_str),
        )
    })?;
    match dt.timezone {
        Some(tz) => {
            let hours = tz.offset_minutes.abs() / 60;
            let mins = tz.offset_minutes.abs() % 60;
            let neg = tz.offset_minutes < 0;
            let dur = Duration {
                negative: neg,
                years: 0,
                months: 0,
                days: 0,
                hours,
                minutes: mins,
                seconds: 0.0,
            };
            Ok(XdmValue::from_atomic(AtomicValue::Duration(
                dur.to_string(),
            )))
        }
        None => Ok(XdmValue::empty()),
    }
}

pub fn fn_year_from_date<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "year-from-date",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let d_str = args[0].to_string_value();
    let d = Date::parse(&d_str).ok_or_else(|| {
        XPath31Error::function("year-from-date", format!("Invalid date: {}", d_str))
    })?;
    Ok(XdmValue::from_integer(d.year as i64))
}

pub fn fn_month_from_date<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "month-from-date",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let d_str = args[0].to_string_value();
    let d = Date::parse(&d_str).ok_or_else(|| {
        XPath31Error::function("month-from-date", format!("Invalid date: {}", d_str))
    })?;
    Ok(XdmValue::from_integer(d.month as i64))
}

pub fn fn_day_from_date<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "day-from-date",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let d_str = args[0].to_string_value();
    let d = Date::parse(&d_str).ok_or_else(|| {
        XPath31Error::function("day-from-date", format!("Invalid date: {}", d_str))
    })?;
    Ok(XdmValue::from_integer(d.day as i64))
}

pub fn fn_timezone_from_date<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "timezone-from-date",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let d_str = args[0].to_string_value();
    let d = Date::parse(&d_str).ok_or_else(|| {
        XPath31Error::function("timezone-from-date", format!("Invalid date: {}", d_str))
    })?;
    match d.timezone {
        Some(tz) => {
            let hours = tz.offset_minutes.abs() / 60;
            let mins = tz.offset_minutes.abs() % 60;
            let neg = tz.offset_minutes < 0;
            let dur = Duration {
                negative: neg,
                years: 0,
                months: 0,
                days: 0,
                hours,
                minutes: mins,
                seconds: 0.0,
            };
            Ok(XdmValue::from_atomic(AtomicValue::Duration(
                dur.to_string(),
            )))
        }
        None => Ok(XdmValue::empty()),
    }
}

pub fn fn_hours_from_time<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "hours-from-time",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let t_str = args[0].to_string_value();
    let t = Time::parse(&t_str).ok_or_else(|| {
        XPath31Error::function("hours-from-time", format!("Invalid time: {}", t_str))
    })?;
    Ok(XdmValue::from_integer(t.hour as i64))
}

pub fn fn_minutes_from_time<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "minutes-from-time",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let t_str = args[0].to_string_value();
    let t = Time::parse(&t_str).ok_or_else(|| {
        XPath31Error::function("minutes-from-time", format!("Invalid time: {}", t_str))
    })?;
    Ok(XdmValue::from_integer(t.minute as i64))
}

pub fn fn_seconds_from_time<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "seconds-from-time",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let t_str = args[0].to_string_value();
    let t = Time::parse(&t_str).ok_or_else(|| {
        XPath31Error::function("seconds-from-time", format!("Invalid time: {}", t_str))
    })?;
    Ok(XdmValue::from_double(t.second))
}

pub fn fn_timezone_from_time<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "timezone-from-time",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let t_str = args[0].to_string_value();
    let t = Time::parse(&t_str).ok_or_else(|| {
        XPath31Error::function("timezone-from-time", format!("Invalid time: {}", t_str))
    })?;
    match t.timezone {
        Some(tz) => {
            let hours = tz.offset_minutes.abs() / 60;
            let mins = tz.offset_minutes.abs() % 60;
            let neg = tz.offset_minutes < 0;
            let dur = Duration {
                negative: neg,
                years: 0,
                months: 0,
                days: 0,
                hours,
                minutes: mins,
                seconds: 0.0,
            };
            Ok(XdmValue::from_atomic(AtomicValue::Duration(
                dur.to_string(),
            )))
        }
        None => Ok(XdmValue::empty()),
    }
}

pub fn fn_years_from_duration<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "years-from-duration",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let d_str = args[0].to_string_value();
    let d = Duration::parse(&d_str).ok_or_else(|| {
        XPath31Error::function(
            "years-from-duration",
            format!("Invalid duration: {}", d_str),
        )
    })?;
    let years = if d.negative { -d.years } else { d.years };
    Ok(XdmValue::from_integer(years as i64))
}

pub fn fn_months_from_duration<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "months-from-duration",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let d_str = args[0].to_string_value();
    let d = Duration::parse(&d_str).ok_or_else(|| {
        XPath31Error::function(
            "months-from-duration",
            format!("Invalid duration: {}", d_str),
        )
    })?;
    let months = if d.negative { -d.months } else { d.months };
    Ok(XdmValue::from_integer(months as i64))
}

pub fn fn_days_from_duration<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "days-from-duration",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let d_str = args[0].to_string_value();
    let d = Duration::parse(&d_str).ok_or_else(|| {
        XPath31Error::function("days-from-duration", format!("Invalid duration: {}", d_str))
    })?;
    let days = if d.negative { -d.days } else { d.days };
    Ok(XdmValue::from_integer(days as i64))
}

pub fn fn_hours_from_duration<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "hours-from-duration",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let d_str = args[0].to_string_value();
    let d = Duration::parse(&d_str).ok_or_else(|| {
        XPath31Error::function(
            "hours-from-duration",
            format!("Invalid duration: {}", d_str),
        )
    })?;
    let hours = if d.negative { -d.hours } else { d.hours };
    Ok(XdmValue::from_integer(hours as i64))
}

pub fn fn_minutes_from_duration<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "minutes-from-duration",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let d_str = args[0].to_string_value();
    let d = Duration::parse(&d_str).ok_or_else(|| {
        XPath31Error::function(
            "minutes-from-duration",
            format!("Invalid duration: {}", d_str),
        )
    })?;
    let minutes = if d.negative { -d.minutes } else { d.minutes };
    Ok(XdmValue::from_integer(minutes as i64))
}

pub fn fn_seconds_from_duration<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "seconds-from-duration",
            "expects exactly 1 argument",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let d_str = args[0].to_string_value();
    let d = Duration::parse(&d_str).ok_or_else(|| {
        XPath31Error::function(
            "seconds-from-duration",
            format!("Invalid duration: {}", d_str),
        )
    })?;
    let seconds = if d.negative { -d.seconds } else { d.seconds };
    Ok(XdmValue::from_double(seconds))
}

pub fn fn_adjust_datetime_to_timezone<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "adjust-dateTime-to-timezone",
            "expects 1 or 2 arguments",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let dt_str = args[0].to_string_value();
    let mut dt = DateTime::parse(&dt_str).ok_or_else(|| {
        XPath31Error::function(
            "adjust-dateTime-to-timezone",
            format!("Invalid dateTime: {}", dt_str),
        )
    })?;

    if args.len() == 1 {
        dt.timezone = Some(Timezone { offset_minutes: 0 });
    } else if args[1].is_empty() {
        dt.timezone = None;
    } else {
        let tz_str = args[1].to_string_value();
        let dur = Duration::parse_day_time(&tz_str).ok_or_else(|| {
            XPath31Error::function(
                "adjust-dateTime-to-timezone",
                format!("Invalid timezone: {}", tz_str),
            )
        })?;
        let offset_minutes = dur.hours * 60 + dur.minutes;
        let offset = if dur.negative {
            -offset_minutes
        } else {
            offset_minutes
        };
        if offset.abs() > 14 * 60 {
            return Err(XPath31Error::function(
                "adjust-dateTime-to-timezone",
                "Timezone offset out of range (-14:00 to +14:00)",
            ));
        }
        dt.timezone = Some(Timezone {
            offset_minutes: offset,
        });
    }
    Ok(XdmValue::from_atomic(AtomicValue::DateTime(dt.to_string())))
}

pub fn fn_adjust_date_to_timezone<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "adjust-date-to-timezone",
            "expects 1 or 2 arguments",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let d_str = args[0].to_string_value();
    let mut d = Date::parse(&d_str).ok_or_else(|| {
        XPath31Error::function(
            "adjust-date-to-timezone",
            format!("Invalid date: {}", d_str),
        )
    })?;

    if args.len() == 1 {
        d.timezone = Some(Timezone { offset_minutes: 0 });
    } else if args[1].is_empty() {
        d.timezone = None;
    } else {
        let tz_str = args[1].to_string_value();
        let dur = Duration::parse_day_time(&tz_str).ok_or_else(|| {
            XPath31Error::function(
                "adjust-date-to-timezone",
                format!("Invalid timezone: {}", tz_str),
            )
        })?;
        let offset_minutes = dur.hours * 60 + dur.minutes;
        let offset = if dur.negative {
            -offset_minutes
        } else {
            offset_minutes
        };
        if offset.abs() > 14 * 60 {
            return Err(XPath31Error::function(
                "adjust-date-to-timezone",
                "Timezone offset out of range",
            ));
        }
        d.timezone = Some(Timezone {
            offset_minutes: offset,
        });
    }
    Ok(XdmValue::from_atomic(AtomicValue::Date(d.to_string())))
}

pub fn fn_adjust_time_to_timezone<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "adjust-time-to-timezone",
            "expects 1 or 2 arguments",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }
    let t_str = args[0].to_string_value();
    let mut t = Time::parse(&t_str).ok_or_else(|| {
        XPath31Error::function(
            "adjust-time-to-timezone",
            format!("Invalid time: {}", t_str),
        )
    })?;

    if args.len() == 1 {
        t.timezone = Some(Timezone { offset_minutes: 0 });
    } else if args[1].is_empty() {
        t.timezone = None;
    } else {
        let tz_str = args[1].to_string_value();
        let dur = Duration::parse_day_time(&tz_str).ok_or_else(|| {
            XPath31Error::function(
                "adjust-time-to-timezone",
                format!("Invalid timezone: {}", tz_str),
            )
        })?;
        let offset_minutes = dur.hours * 60 + dur.minutes;
        let offset = if dur.negative {
            -offset_minutes
        } else {
            offset_minutes
        };
        if offset.abs() > 14 * 60 {
            return Err(XPath31Error::function(
                "adjust-time-to-timezone",
                "Timezone offset out of range",
            ));
        }
        t.timezone = Some(Timezone {
            offset_minutes: offset,
        });
    }
    Ok(XdmValue::from_atomic(AtomicValue::Time(t.to_string())))
}

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

const DAY_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

struct DateTimeComponents<'a> {
    year: i32,
    month: u8,
    day: u8,
    hour: Option<u8>,
    minute: Option<u8>,
    second: Option<f64>,
    timezone: &'a Option<Timezone>,
}

fn day_of_week(year: i32, month: u8, day: u8) -> usize {
    let y = if month < 3 { year - 1 } else { year };
    let m = if month < 3 { month + 12 } else { month };
    let q = day as i32;
    let k = y % 100;
    let j = y / 100;
    let h = (q + (13 * (m as i32 + 1)) / 5 + k + k / 4 + j / 4 - 2 * j) % 7;
    ((h + 6) % 7) as usize
}

fn format_datetime_picture(dt: &DateTimeComponents, picture: &str) -> Result<String, XPath31Error> {
    let mut result = String::new();
    let mut chars = picture.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '[' {
            let mut component = String::new();
            while let Some(&c) = chars.peek() {
                if c == ']' {
                    chars.next();
                    break;
                }
                // Safe: peek() returned Some, so next() will return Some
                if let Some(next_char) = chars.next() {
                    component.push(next_char);
                }
            }
            result.push_str(&format_component(&component, dt)?);
        } else if ch == ']' {
            continue;
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

fn format_component(component: &str, dt: &DateTimeComponents) -> Result<String, XPath31Error> {
    let component = component.trim();
    if component.is_empty() {
        return Ok(String::new());
    }

    let specifier = match component.chars().next() {
        Some(c) => c,
        None => return Ok(String::new()),
    };
    let format = &component[1..];

    match specifier {
        'Y' => format_year(dt.year, format),
        'M' => format_month(dt.month, format),
        'D' => format_day(dt.day, format),
        'H' => format_hour_24(dt.hour.unwrap_or(0), format),
        'h' => format_hour_12(dt.hour.unwrap_or(0), format),
        'm' => format_minute(dt.minute.unwrap_or(0), format),
        's' => format_second(dt.second.unwrap_or(0.0), format),
        'P' => format_ampm(dt.hour.unwrap_or(0), format),
        'F' => format_day_of_week(dt.year, dt.month, dt.day, format),
        'Z' | 'z' => format_timezone(dt.timezone, format),
        'd' => format_day_in_year(dt.year, dt.month, dt.day, format),
        'W' => format_week_in_year(dt.year, dt.month, dt.day, format),
        'w' => format_week_in_month(dt.day, format),
        'C' => Ok("ISO".to_string()),
        'E' => Ok(dt.year.to_string()),
        _ => Ok(component.to_string()),
    }
}

fn format_year(year: i32, format: &str) -> Result<String, XPath31Error> {
    let abs_year = year.abs();
    if format.contains("0001") || format.is_empty() {
        Ok(format!("{:04}", abs_year))
    } else if format.contains("01") {
        Ok(format!("{:02}", abs_year % 100))
    } else if format.contains('1') {
        Ok(abs_year.to_string())
    } else {
        Ok(format!("{:04}", abs_year))
    }
}

fn format_month(month: u8, format: &str) -> Result<String, XPath31Error> {
    if format.contains('N') || format.contains('n') {
        let name = MONTH_NAMES.get(month as usize - 1).unwrap_or(&"");
        if format.contains("Nn") {
            Ok(name.to_string())
        } else if format.contains('n') {
            Ok(name.to_lowercase())
        } else {
            Ok(name.to_uppercase())
        }
    } else if format.contains("01") || format.is_empty() {
        Ok(format!("{:02}", month))
    } else {
        Ok(month.to_string())
    }
}

fn format_day(day: u8, format: &str) -> Result<String, XPath31Error> {
    if format.contains("01") || format.is_empty() {
        Ok(format!("{:02}", day))
    } else if format.contains('o') {
        Ok(format!("{}{}", day, ordinal_suffix(day as i32)))
    } else {
        Ok(day.to_string())
    }
}

fn format_hour_24(hour: u8, format: &str) -> Result<String, XPath31Error> {
    if format.contains("01") || format.is_empty() {
        Ok(format!("{:02}", hour))
    } else {
        Ok(hour.to_string())
    }
}

fn format_hour_12(hour: u8, format: &str) -> Result<String, XPath31Error> {
    let h12 = if hour == 0 {
        12
    } else if hour > 12 {
        hour - 12
    } else {
        hour
    };
    if format.contains("01") {
        Ok(format!("{:02}", h12))
    } else {
        Ok(h12.to_string())
    }
}

fn format_minute(minute: u8, format: &str) -> Result<String, XPath31Error> {
    if format.contains("01") || format.is_empty() {
        Ok(format!("{:02}", minute))
    } else {
        Ok(minute.to_string())
    }
}

fn format_second(second: f64, format: &str) -> Result<String, XPath31Error> {
    if format.contains("01") || format.is_empty() {
        Ok(format!("{:02}", second as u8))
    } else {
        Ok((second as u8).to_string())
    }
}

fn format_ampm(hour: u8, format: &str) -> Result<String, XPath31Error> {
    let is_pm = hour >= 12;
    if format.contains('n') {
        Ok(if is_pm { "p.m." } else { "a.m." }.to_string())
    } else {
        Ok(if is_pm { "PM" } else { "AM" }.to_string())
    }
}

fn format_day_of_week(year: i32, month: u8, day: u8, format: &str) -> Result<String, XPath31Error> {
    let dow = day_of_week(year, month, day);
    let name = DAY_NAMES[dow];
    if format.contains('N') || format.contains('n') {
        if format.contains("Nn") {
            Ok(name.to_string())
        } else if format.contains('n') {
            Ok(name.to_lowercase())
        } else {
            Ok(name.to_uppercase())
        }
    } else {
        Ok((dow + 1).to_string())
    }
}

fn format_timezone(tz: &Option<Timezone>, _format: &str) -> Result<String, XPath31Error> {
    match tz {
        Some(t) => Ok(t.to_string()),
        None => Ok(String::new()),
    }
}

fn format_day_in_year(year: i32, month: u8, day: u8, format: &str) -> Result<String, XPath31Error> {
    let days_before = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let mut doy = days_before.get(month as usize - 1).copied().unwrap_or(0) + day as i32;
    if month > 2 && is_leap_year(year) {
        doy += 1;
    }
    if format.contains("001") {
        Ok(format!("{:03}", doy))
    } else {
        Ok(doy.to_string())
    }
}

fn format_week_in_year(
    _year: i32,
    _month: u8,
    _day: u8,
    format: &str,
) -> Result<String, XPath31Error> {
    if format.contains("01") {
        Ok("01".to_string())
    } else {
        Ok("1".to_string())
    }
}

fn format_week_in_month(day: u8, _format: &str) -> Result<String, XPath31Error> {
    Ok(((day - 1) / 7 + 1).to_string())
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn ordinal_suffix(n: i32) -> &'static str {
    let abs_n = n.abs();
    if (11..=13).contains(&(abs_n % 100)) {
        "th"
    } else {
        match abs_n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    }
}

pub fn fn_implicit_timezone<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if !args.is_empty() {
        return Err(XPath31Error::function(
            "implicit-timezone",
            "expects no arguments",
        ));
    }

    let dur = Duration {
        negative: false,
        years: 0,
        months: 0,
        days: 0,
        hours: 0,
        minutes: 0,
        seconds: 0.0,
    };
    Ok(XdmValue::from_atomic(AtomicValue::Duration(
        dur.to_string(),
    )))
}

pub fn fn_format_datetime<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 5 {
        return Err(XPath31Error::function(
            "format-dateTime",
            "expects 2 to 5 arguments",
        ));
    }

    if args[0].is_empty() {
        return Ok(XdmValue::from_string(String::new()));
    }

    let dt_str = args[0].to_string_value();
    let picture = args[1].to_string_value();

    let dt = DateTime::parse(&dt_str).ok_or_else(|| {
        XPath31Error::function("format-dateTime", format!("Invalid dateTime: {}", dt_str))
    })?;

    let components = DateTimeComponents {
        year: dt.year,
        month: dt.month,
        day: dt.day,
        hour: Some(dt.hour),
        minute: Some(dt.minute),
        second: Some(dt.second),
        timezone: &dt.timezone,
    };

    let result = format_datetime_picture(&components, &picture)?;
    Ok(XdmValue::from_string(result))
}

pub fn fn_format_date<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 5 {
        return Err(XPath31Error::function(
            "format-date",
            "expects 2 to 5 arguments",
        ));
    }

    if args[0].is_empty() {
        return Ok(XdmValue::from_string(String::new()));
    }

    let d_str = args[0].to_string_value();
    let picture = args[1].to_string_value();

    let d = Date::parse(&d_str)
        .ok_or_else(|| XPath31Error::function("format-date", format!("Invalid date: {}", d_str)))?;

    let components = DateTimeComponents {
        year: d.year,
        month: d.month,
        day: d.day,
        hour: None,
        minute: None,
        second: None,
        timezone: &d.timezone,
    };

    let result = format_datetime_picture(&components, &picture)?;
    Ok(XdmValue::from_string(result))
}

pub fn fn_format_time<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 5 {
        return Err(XPath31Error::function(
            "format-time",
            "expects 2 to 5 arguments",
        ));
    }

    if args[0].is_empty() {
        return Ok(XdmValue::from_string(String::new()));
    }

    let t_str = args[0].to_string_value();
    let picture = args[1].to_string_value();

    let t = Time::parse(&t_str)
        .ok_or_else(|| XPath31Error::function("format-time", format!("Invalid time: {}", t_str)))?;

    let components = DateTimeComponents {
        year: 1970,
        month: 1,
        day: 1,
        hour: Some(t.hour),
        minute: Some(t.minute),
        second: Some(t.second),
        timezone: &t.timezone,
    };

    let result = format_datetime_picture(&components, &picture)?;
    Ok(XdmValue::from_string(result))
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn add_year_month_duration(date: &Date, dur: &Duration) -> Date {
    let sign = if dur.negative { -1 } else { 1 };
    let total_months =
        date.year * 12 + date.month as i32 - 1 + sign * (dur.years * 12 + dur.months);

    let new_year = total_months.div_euclid(12);
    let new_month = (total_months.rem_euclid(12) + 1) as u8;
    let new_day = date.day.min(days_in_month(new_year, new_month));

    Date {
        year: new_year,
        month: new_month,
        day: new_day,
        timezone: date.timezone.clone(),
    }
}

fn add_day_time_duration(date: &Date, dur: &Duration) -> Date {
    let sign = if dur.negative { -1i64 } else { 1i64 };
    let total_seconds = sign
        * (dur.days as i64 * 86400
            + dur.hours as i64 * 3600
            + dur.minutes as i64 * 60
            + dur.seconds as i64);
    let day_offset = (total_seconds / 86400) as i32;

    let days_from_epoch = ymd_to_days(date.year, date.month, date.day) + day_offset;
    let (new_year, new_month, new_day) = days_to_ymd(days_from_epoch as i64 + 719_468);

    Date {
        year: new_year,
        month: new_month,
        day: new_day,
        timezone: date.timezone.clone(),
    }
}

fn ymd_to_days(year: i32, month: u8, day: u8) -> i32 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y / 400 } else { (y - 399) / 400 };
    let yoe = (y - era * 400) as u32;
    let m = month as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as u32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i32 - 719468
}

pub fn fn_add_yearmonth_duration_to_date<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "add-yearMonthDuration-to-date",
            "expects 2 arguments",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }

    let date_str = args[0].to_string_value();
    let dur_str = args[1].to_string_value();

    let date = Date::parse(&date_str).ok_or_else(|| {
        XPath31Error::function(
            "add-yearMonthDuration-to-date",
            format!("Invalid date: {}", date_str),
        )
    })?;

    let dur = Duration::parse(&dur_str).ok_or_else(|| {
        XPath31Error::function(
            "add-yearMonthDuration-to-date",
            format!("Invalid duration: {}", dur_str),
        )
    })?;

    let result = add_year_month_duration(&date, &dur);
    Ok(XdmValue::from_atomic(AtomicValue::Date(result.to_string())))
}

pub fn fn_add_daytimeduration_to_date<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "add-dayTimeDuration-to-date",
            "expects 2 arguments",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }

    let date_str = args[0].to_string_value();
    let dur_str = args[1].to_string_value();

    let date = Date::parse(&date_str).ok_or_else(|| {
        XPath31Error::function(
            "add-dayTimeDuration-to-date",
            format!("Invalid date: {}", date_str),
        )
    })?;

    let dur = Duration::parse_day_time(&dur_str)
        .or_else(|| Duration::parse(&dur_str))
        .ok_or_else(|| {
            XPath31Error::function(
                "add-dayTimeDuration-to-date",
                format!("Invalid duration: {}", dur_str),
            )
        })?;

    let result = add_day_time_duration(&date, &dur);
    Ok(XdmValue::from_atomic(AtomicValue::Date(result.to_string())))
}

pub fn fn_subtract_dates<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "subtract-dates",
            "expects 2 arguments",
        ));
    }
    if args[0].is_empty() || args[1].is_empty() {
        return Ok(XdmValue::empty());
    }

    let date1_str = args[0].to_string_value();
    let date2_str = args[1].to_string_value();

    let date1 = Date::parse(&date1_str).ok_or_else(|| {
        XPath31Error::function("subtract-dates", format!("Invalid date: {}", date1_str))
    })?;

    let date2 = Date::parse(&date2_str).ok_or_else(|| {
        XPath31Error::function("subtract-dates", format!("Invalid date: {}", date2_str))
    })?;

    let days1 = ymd_to_days(date1.year, date1.month, date1.day);
    let days2 = ymd_to_days(date2.year, date2.month, date2.day);
    let diff_days = days1 - days2;

    let dur = Duration {
        negative: diff_days < 0,
        years: 0,
        months: 0,
        days: diff_days.abs(),
        hours: 0,
        minutes: 0,
        seconds: 0.0,
    };

    Ok(XdmValue::from_atomic(AtomicValue::Duration(
        dur.to_string(),
    )))
}

pub fn fn_add_yearmonth_duration_to_datetime<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "add-yearMonthDuration-to-dateTime",
            "expects 2 arguments",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }

    let dt_str = args[0].to_string_value();
    let dur_str = args[1].to_string_value();

    let dt = DateTime::parse(&dt_str).ok_or_else(|| {
        XPath31Error::function(
            "add-yearMonthDuration-to-dateTime",
            format!("Invalid dateTime: {}", dt_str),
        )
    })?;

    let dur = Duration::parse(&dur_str).ok_or_else(|| {
        XPath31Error::function(
            "add-yearMonthDuration-to-dateTime",
            format!("Invalid duration: {}", dur_str),
        )
    })?;

    let date = Date {
        year: dt.year,
        month: dt.month,
        day: dt.day,
        timezone: dt.timezone.clone(),
    };
    let new_date = add_year_month_duration(&date, &dur);

    let result = DateTime {
        year: new_date.year,
        month: new_date.month,
        day: new_date.day,
        hour: dt.hour,
        minute: dt.minute,
        second: dt.second,
        timezone: dt.timezone,
    };

    Ok(XdmValue::from_atomic(AtomicValue::DateTime(
        result.to_string(),
    )))
}

pub fn fn_add_daytimeduration_to_datetime<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "add-dayTimeDuration-to-dateTime",
            "expects 2 arguments",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }

    let dt_str = args[0].to_string_value();
    let dur_str = args[1].to_string_value();

    let dt = DateTime::parse(&dt_str).ok_or_else(|| {
        XPath31Error::function(
            "add-dayTimeDuration-to-dateTime",
            format!("Invalid dateTime: {}", dt_str),
        )
    })?;

    let dur = Duration::parse_day_time(&dur_str)
        .or_else(|| Duration::parse(&dur_str))
        .ok_or_else(|| {
            XPath31Error::function(
                "add-dayTimeDuration-to-dateTime",
                format!("Invalid duration: {}", dur_str),
            )
        })?;

    let sign = if dur.negative { -1i64 } else { 1i64 };
    let total_seconds = sign
        * (dur.days as i64 * 86400
            + dur.hours as i64 * 3600
            + dur.minutes as i64 * 60
            + dur.seconds as i64);

    let current_seconds = dt.hour as i64 * 3600 + dt.minute as i64 * 60 + dt.second as i64;
    let new_total_seconds = current_seconds + total_seconds;

    let day_offset = new_total_seconds.div_euclid(86400) as i32;
    let time_seconds = new_total_seconds.rem_euclid(86400);

    let new_hour = (time_seconds / 3600) as u8;
    let new_minute = ((time_seconds % 3600) / 60) as u8;
    let new_second = (time_seconds % 60) as f64 + dt.second.fract();

    let date = Date {
        year: dt.year,
        month: dt.month,
        day: dt.day,
        timezone: dt.timezone.clone(),
    };
    let dur_days = Duration {
        negative: day_offset < 0,
        years: 0,
        months: 0,
        days: day_offset.abs(),
        hours: 0,
        minutes: 0,
        seconds: 0.0,
    };
    let new_date = add_day_time_duration(&date, &dur_days);

    let result = DateTime {
        year: new_date.year,
        month: new_date.month,
        day: new_date.day,
        hour: new_hour,
        minute: new_minute,
        second: new_second,
        timezone: dt.timezone,
    };

    Ok(XdmValue::from_atomic(AtomicValue::DateTime(
        result.to_string(),
    )))
}

pub fn fn_subtract_datetimes<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "subtract-dateTimes",
            "expects 2 arguments",
        ));
    }
    if args[0].is_empty() || args[1].is_empty() {
        return Ok(XdmValue::empty());
    }

    let dt1_str = args[0].to_string_value();
    let dt2_str = args[1].to_string_value();

    let dt1 = DateTime::parse(&dt1_str).ok_or_else(|| {
        XPath31Error::function(
            "subtract-dateTimes",
            format!("Invalid dateTime: {}", dt1_str),
        )
    })?;

    let dt2 = DateTime::parse(&dt2_str).ok_or_else(|| {
        XPath31Error::function(
            "subtract-dateTimes",
            format!("Invalid dateTime: {}", dt2_str),
        )
    })?;

    let days1 = ymd_to_days(dt1.year, dt1.month, dt1.day);
    let days2 = ymd_to_days(dt2.year, dt2.month, dt2.day);

    let secs1 = dt1.hour as i64 * 3600 + dt1.minute as i64 * 60 + dt1.second as i64;
    let secs2 = dt2.hour as i64 * 3600 + dt2.minute as i64 * 60 + dt2.second as i64;

    let total_secs1 = days1 as i64 * 86400 + secs1;
    let total_secs2 = days2 as i64 * 86400 + secs2;
    let diff_secs = total_secs1 - total_secs2;

    let negative = diff_secs < 0;
    let abs_secs = diff_secs.abs();

    let days = (abs_secs / 86400) as i32;
    let rem = abs_secs % 86400;
    let hours = (rem / 3600) as i32;
    let rem = rem % 3600;
    let minutes = (rem / 60) as i32;
    let seconds = (rem % 60) as f64;

    let dur = Duration {
        negative,
        years: 0,
        months: 0,
        days,
        hours,
        minutes,
        seconds,
    };

    Ok(XdmValue::from_atomic(AtomicValue::Duration(
        dur.to_string(),
    )))
}

pub fn fn_add_daytimeduration_to_time<N: Clone>(
    args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "add-dayTimeDuration-to-time",
            "expects 2 arguments",
        ));
    }
    if args[0].is_empty() {
        return Ok(XdmValue::empty());
    }

    let t_str = args[0].to_string_value();
    let dur_str = args[1].to_string_value();

    let t = Time::parse(&t_str).ok_or_else(|| {
        XPath31Error::function(
            "add-dayTimeDuration-to-time",
            format!("Invalid time: {}", t_str),
        )
    })?;

    let dur = Duration::parse_day_time(&dur_str)
        .or_else(|| Duration::parse(&dur_str))
        .ok_or_else(|| {
            XPath31Error::function(
                "add-dayTimeDuration-to-time",
                format!("Invalid duration: {}", dur_str),
            )
        })?;

    let sign = if dur.negative { -1i64 } else { 1i64 };
    let dur_secs = sign
        * (dur.days as i64 * 86400
            + dur.hours as i64 * 3600
            + dur.minutes as i64 * 60
            + dur.seconds as i64);

    let current_secs = t.hour as i64 * 3600 + t.minute as i64 * 60 + t.second as i64;
    let new_secs = (current_secs + dur_secs).rem_euclid(86400);

    let new_hour = (new_secs / 3600) as u8;
    let new_minute = ((new_secs % 3600) / 60) as u8;
    let new_second = (new_secs % 60) as f64 + t.second.fract();

    let result = Time {
        hour: new_hour,
        minute: new_minute,
        second: new_second,
        timezone: t.timezone,
    };

    Ok(XdmValue::from_atomic(AtomicValue::Time(result.to_string())))
}

pub fn fn_subtract_times<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "subtract-times",
            "expects 2 arguments",
        ));
    }
    if args[0].is_empty() || args[1].is_empty() {
        return Ok(XdmValue::empty());
    }

    let t1_str = args[0].to_string_value();
    let t2_str = args[1].to_string_value();

    let t1 = Time::parse(&t1_str).ok_or_else(|| {
        XPath31Error::function("subtract-times", format!("Invalid time: {}", t1_str))
    })?;

    let t2 = Time::parse(&t2_str).ok_or_else(|| {
        XPath31Error::function("subtract-times", format!("Invalid time: {}", t2_str))
    })?;

    let secs1 = t1.hour as i64 * 3600 + t1.minute as i64 * 60 + t1.second as i64;
    let secs2 = t2.hour as i64 * 3600 + t2.minute as i64 * 60 + t2.second as i64;
    let diff = secs1 - secs2;

    let negative = diff < 0;
    let abs_diff = diff.abs();

    let hours = (abs_diff / 3600) as i32;
    let rem = abs_diff % 3600;
    let minutes = (rem / 60) as i32;
    let seconds = (rem % 60) as f64;

    let dur = Duration {
        negative,
        years: 0,
        months: 0,
        days: 0,
        hours,
        minutes,
        seconds,
    };

    Ok(XdmValue::from_atomic(AtomicValue::Duration(
        dur.to_string(),
    )))
}

static IETF_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:(?:Mon|Tue|Wed|Thu|Fri|Sat|Sun),?\s+)?(\d{1,2})\s+(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+(\d{2,4})\s+(\d{2}):(\d{2})(?::(\d{2}))?\s*([A-Z]{2,4}|[+-]\d{4})?$"
    ).unwrap()
});

pub fn fn_parse_ietf_date<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "parse-ietf-date",
            "Expected 1 argument",
        ));
    }

    let input = args.remove(0);
    if input.is_empty() {
        return Ok(XdmValue::empty());
    }

    let input_str = input.to_string_value();
    let caps = IETF_DATE_RE.captures(input_str.trim()).ok_or_else(|| {
        XPath31Error::function(
            "parse-ietf-date",
            format!("Invalid IETF date format: {}", input_str),
        )
    })?;

    let day: u8 = caps
        .get(1)
        .unwrap()
        .as_str()
        .parse()
        .map_err(|_| XPath31Error::function("parse-ietf-date", "Invalid day"))?;

    let month_str = caps.get(2).unwrap().as_str().to_lowercase();
    let month: u8 = match month_str.as_str() {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return Err(XPath31Error::function("parse-ietf-date", "Invalid month")),
    };

    let year_str = caps.get(3).unwrap().as_str();
    let year: i32 = year_str
        .parse()
        .map_err(|_| XPath31Error::function("parse-ietf-date", "Invalid year"))?;
    let year = if year < 100 {
        if year < 50 { 2000 + year } else { 1900 + year }
    } else {
        year
    };

    let hour: u8 = caps
        .get(4)
        .unwrap()
        .as_str()
        .parse()
        .map_err(|_| XPath31Error::function("parse-ietf-date", "Invalid hour"))?;

    let minute: u8 = caps
        .get(5)
        .unwrap()
        .as_str()
        .parse()
        .map_err(|_| XPath31Error::function("parse-ietf-date", "Invalid minute"))?;

    let second: f64 = caps
        .get(6)
        .map(|m| m.as_str().parse().unwrap_or(0.0))
        .unwrap_or(0.0);

    let timezone = caps.get(7).and_then(|m| {
        let tz_str = m.as_str().to_uppercase();
        let offset = match tz_str.as_str() {
            "GMT" | "UTC" | "UT" | "Z" => 0,
            "EST" => -5 * 60,
            "EDT" => -4 * 60,
            "CST" => -6 * 60,
            "CDT" => -5 * 60,
            "MST" => -7 * 60,
            "MDT" => -6 * 60,
            "PST" => -8 * 60,
            "PDT" => -7 * 60,
            _ if tz_str.starts_with('+') || tz_str.starts_with('-') => {
                let sign = if tz_str.starts_with('-') { -1 } else { 1 };
                let offset_str = tz_str.trim_start_matches(['+', '-']);
                if offset_str.len() == 4 {
                    let hours: i32 = offset_str[..2].parse().unwrap_or(0);
                    let mins: i32 = offset_str[2..].parse().unwrap_or(0);
                    sign * (hours * 60 + mins)
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        Some(Timezone {
            offset_minutes: offset,
        })
    });

    let dt = DateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
        timezone,
    };

    Ok(XdmValue::from_atomic(AtomicValue::DateTime(dt.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_datetime() {
        let dt = DateTime::parse("2024-03-15T10:30:45Z").unwrap();
        assert_eq!(dt.year, 2024);
        assert_eq!(dt.month, 3);
        assert_eq!(dt.day, 15);
        assert_eq!(dt.hour, 10);
        assert_eq!(dt.minute, 30);
        assert_eq!(dt.second, 45.0);
        assert_eq!(dt.timezone.as_ref().unwrap().offset_minutes, 0);

        let dt2 = DateTime::parse("2024-03-15T10:30:45.123+05:30").unwrap();
        assert_eq!(dt2.second, 45.123);
        assert_eq!(dt2.timezone.as_ref().unwrap().offset_minutes, 330);

        let dt3 = DateTime::parse("2024-03-15T10:30:45-08:00").unwrap();
        assert_eq!(dt3.timezone.as_ref().unwrap().offset_minutes, -480);
    }

    #[test]
    fn test_parse_date() {
        let d = Date::parse("2024-03-15").unwrap();
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, 3);
        assert_eq!(d.day, 15);
        assert!(d.timezone.is_none());

        let d2 = Date::parse("2024-03-15Z").unwrap();
        assert_eq!(d2.timezone.as_ref().unwrap().offset_minutes, 0);
    }

    #[test]
    fn test_parse_time() {
        let t = Time::parse("10:30:45").unwrap();
        assert_eq!(t.hour, 10);
        assert_eq!(t.minute, 30);
        assert_eq!(t.second, 45.0);

        let t2 = Time::parse("10:30:45.5Z").unwrap();
        assert_eq!(t2.second, 45.5);
    }

    #[test]
    fn test_parse_duration() {
        let d = Duration::parse("P1Y2M3DT4H5M6S").unwrap();
        assert!(!d.negative);
        assert_eq!(d.years, 1);
        assert_eq!(d.months, 2);
        assert_eq!(d.days, 3);
        assert_eq!(d.hours, 4);
        assert_eq!(d.minutes, 5);
        assert_eq!(d.seconds, 6.0);

        let d2 = Duration::parse("-P1Y").unwrap();
        assert!(d2.negative);
        assert_eq!(d2.years, 1);
    }

    #[test]
    fn test_duration_to_string() {
        let d = Duration {
            negative: false,
            years: 1,
            months: 2,
            days: 3,
            hours: 4,
            minutes: 5,
            seconds: 6.0,
        };
        assert_eq!(d.to_string(), "P1Y2M3DT4H5M6S");

        let d2 = Duration {
            negative: true,
            years: 0,
            months: 0,
            days: 0,
            hours: 0,
            minutes: 0,
            seconds: 0.0,
        };
        assert_eq!(d2.to_string(), "PT0S");
    }

    #[test]
    fn test_datetime_to_string() {
        let dt = DateTime {
            year: 2024,
            month: 3,
            day: 15,
            hour: 10,
            minute: 30,
            second: 45.0,
            timezone: Some(Timezone { offset_minutes: 0 }),
        };
        assert_eq!(dt.to_string(), "2024-03-15T10:30:45Z");
    }

    #[test]
    fn test_current_datetime_returns_real_time() {
        let result: XdmValue<()> = fn_current_datetime(vec![]).unwrap();
        let dt_str = result.to_string_value();
        let dt = DateTime::parse(&dt_str).expect("current-dateTime should return valid dateTime");
        assert!(dt.year >= 2024, "Year should be 2024 or later");
        assert!(dt.timezone.is_some(), "Should have UTC timezone");
    }

    #[test]
    fn test_current_date_returns_real_date() {
        let result: XdmValue<()> = fn_current_date(vec![]).unwrap();
        let d_str = result.to_string_value();
        let d = Date::parse(&d_str).expect("current-date should return valid date");
        assert!(d.year >= 2024, "Year should be 2024 or later");
    }

    #[test]
    fn test_current_time_returns_valid_time() {
        let result: XdmValue<()> = fn_current_time(vec![]).unwrap();
        let t_str = result.to_string_value();
        let t = Time::parse(&t_str).expect("current-time should return valid time");
        assert!(t.hour <= 23, "Hour should be valid");
        assert!(t.minute <= 59, "Minute should be valid");
    }

    #[test]
    fn test_format_datetime() {
        let result: XdmValue<()> = fn_format_datetime(vec![
            XdmValue::from_string("2024-03-15T14:30:45Z".to_string()),
            XdmValue::from_string("[Y0001]-[M01]-[D01]".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "2024-03-15");
    }

    #[test]
    fn test_format_datetime_with_time() {
        let result: XdmValue<()> = fn_format_datetime(vec![
            XdmValue::from_string("2024-03-15T14:30:45Z".to_string()),
            XdmValue::from_string("[H01]:[m01]:[s01]".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "14:30:45");
    }

    #[test]
    fn test_format_date() {
        let result: XdmValue<()> = fn_format_date(vec![
            XdmValue::from_string("2024-03-15".to_string()),
            XdmValue::from_string("[MNn] [D], [Y]".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "March 15, 2024");
    }

    #[test]
    fn test_format_time() {
        let result: XdmValue<()> = fn_format_time(vec![
            XdmValue::from_string("14:30:45".to_string()),
            XdmValue::from_string("[h]:[m01] [P]".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "2:30 PM");
    }

    #[test]
    fn test_format_date_day_of_week() {
        let result: XdmValue<()> = fn_format_date(vec![
            XdmValue::from_string("2024-03-15".to_string()),
            XdmValue::from_string("[FNn], [MNn] [D]".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "Friday, March 15");
    }

    #[test]
    fn test_format_date_empty_component() {
        let result: XdmValue<()> = fn_format_date(vec![
            XdmValue::from_string("2024-03-15".to_string()),
            XdmValue::from_string("[]".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "");
    }

    #[test]
    fn test_format_date_unclosed_bracket() {
        let result: XdmValue<()> = fn_format_date(vec![
            XdmValue::from_string("2024-03-15".to_string()),
            XdmValue::from_string("[Y".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "2024");
    }

    #[test]
    fn test_format_date_whitespace_component() {
        let result: XdmValue<()> = fn_format_date(vec![
            XdmValue::from_string("2024-03-15".to_string()),
            XdmValue::from_string("[   ]".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "");
    }
}
