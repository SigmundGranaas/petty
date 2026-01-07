use crate::engine::EvaluationContext;
use crate::error::XPath31Error;
use crate::types::*;
use petty_xpath1::DataSourceNode;

pub fn fn_abs<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("abs", "Expected 1 argument"));
    }
    let n = args.remove(0).to_double();
    Ok(XdmValue::from_double(n.abs()))
}

pub fn fn_ceiling<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("ceiling", "Expected 1 argument"));
    }
    let n = args.remove(0).to_double();
    Ok(XdmValue::from_double(n.ceil()))
}

pub fn fn_floor<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("floor", "Expected 1 argument"));
    }
    let n = args.remove(0).to_double();
    Ok(XdmValue::from_double(n.floor()))
}

pub fn fn_round<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function("round", "Expected 1 or 2 arguments"));
    }

    let precision = if args.len() == 2 {
        args.remove(1).to_double() as i32
    } else {
        0
    };

    let n = args.remove(0).to_double();

    if n.is_nan() || n.is_infinite() || n == 0.0 {
        return Ok(XdmValue::from_double(n));
    }

    if precision == 0 {
        Ok(XdmValue::from_double((n + 0.5).floor()))
    } else {
        let factor = 10_f64.powi(precision);
        Ok(XdmValue::from_double(((n * factor) + 0.5).floor() / factor))
    }
}

pub fn fn_round_half_to_even<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function(
            "round-half-to-even",
            "Expected 1 or 2 arguments",
        ));
    }

    let precision = if args.len() == 2 {
        args.remove(1).to_double() as i32
    } else {
        0
    };

    let n = args.remove(0).to_double();

    if n.is_nan() || n.is_infinite() || n == 0.0 {
        return Ok(XdmValue::from_double(n));
    }

    let factor = 10_f64.powi(precision);
    let scaled = n * factor;
    let floored = scaled.floor();
    let frac = scaled - floored;

    let rounded = if (frac - 0.5).abs() < f64::EPSILON {
        if floored as i64 % 2 == 0 {
            floored
        } else {
            floored + 1.0
        }
    } else if frac > 0.5 {
        floored + 1.0
    } else {
        floored
    };

    Ok(XdmValue::from_double(rounded / factor))
}

pub fn fn_number<'a, N: DataSourceNode<'a> + Clone>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() > 1 {
        return Err(XPath31Error::function(
            "number",
            "Expected 0 or 1 arguments",
        ));
    }

    let n = if args.is_empty() {
        match &ctx.context_item {
            Some(XdmItem::Atomic(a)) => a.to_double(),
            Some(XdmItem::Node(n)) => n.string_value().parse().unwrap_or(f64::NAN),
            _ => f64::NAN,
        }
    } else {
        args.remove(0).to_double()
    };

    Ok(XdmValue::from_double(n))
}

pub fn fn_sum<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function("sum", "Expected 1 or 2 arguments"));
    }

    let default = if args.len() == 2 {
        Some(args.remove(1).to_double())
    } else {
        None
    };

    let seq = args.remove(0);
    let items = seq.items();

    if items.is_empty() {
        return Ok(XdmValue::from_double(default.unwrap_or(0.0)));
    }

    let mut sum = 0.0;
    for item in items {
        match item {
            XdmItem::Atomic(a) => sum += a.to_double(),
            XdmItem::Node(_) => {
                sum += f64::NAN;
            }
            _ => {}
        }
    }

    Ok(XdmValue::from_double(sum))
}

pub fn fn_avg<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("avg", "Expected 1 argument"));
    }

    let seq = args.remove(0);
    let items = seq.items();

    if items.is_empty() {
        return Ok(XdmValue::empty());
    }

    let mut sum = 0.0;
    let mut count = 0;

    for item in items {
        if let XdmItem::Atomic(a) = item {
            sum += a.to_double();
            count += 1;
        }
    }

    if count == 0 {
        return Ok(XdmValue::empty());
    }

    Ok(XdmValue::from_double(sum / count as f64))
}

pub fn fn_min<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function("min", "Expected 1 or 2 arguments"));
    }

    let seq = args.remove(0);
    let items = seq.items();

    if items.is_empty() {
        return Ok(XdmValue::empty());
    }

    let mut min: Option<f64> = None;

    for item in items {
        if let XdmItem::Atomic(a) = item {
            let val = a.to_double();
            if val.is_nan() {
                return Ok(XdmValue::from_double(f64::NAN));
            }
            min = Some(min.map(|m| m.min(val)).unwrap_or(val));
        }
    }

    match min {
        Some(m) => Ok(XdmValue::from_double(m)),
        None => Ok(XdmValue::empty()),
    }
}

pub fn fn_max<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 2 {
        return Err(XPath31Error::function("max", "Expected 1 or 2 arguments"));
    }

    let seq = args.remove(0);
    let items = seq.items();

    if items.is_empty() {
        return Ok(XdmValue::empty());
    }

    let mut max: Option<f64> = None;

    for item in items {
        if let XdmItem::Atomic(a) = item {
            let val = a.to_double();
            if val.is_nan() {
                return Ok(XdmValue::from_double(f64::NAN));
            }
            max = Some(max.map(|m| m.max(val)).unwrap_or(val));
        }
    }

    match max {
        Some(m) => Ok(XdmValue::from_double(m)),
        None => Ok(XdmValue::empty()),
    }
}

/// Decimal format settings for fn:format-number
#[derive(Debug, Clone)]
pub struct DecimalFormat {
    pub decimal_separator: char,
    pub grouping_separator: char,
    pub minus_sign: char,
    pub percent: char,
    pub per_mille: char,
    pub zero_digit: char,
    pub digit: char,
    pub pattern_separator: char,
    pub infinity: String,
    pub nan: String,
}

impl Default for DecimalFormat {
    fn default() -> Self {
        Self {
            decimal_separator: '.',
            grouping_separator: ',',
            minus_sign: '-',
            percent: '%',
            per_mille: '‰',
            zero_digit: '0',
            digit: '#',
            pattern_separator: ';',
            infinity: "Infinity".to_string(),
            nan: "NaN".to_string(),
        }
    }
}

/// Parsed picture string for format-number
#[derive(Debug, Default)]
struct NumberPicture {
    prefix: String,
    suffix: String,
    integer_min_digits: usize,
    integer_max_digits: usize,
    fraction_min_digits: usize,
    fraction_max_digits: usize,
    grouping_positions: Vec<usize>,
    uses_grouping: bool,
    is_percent: bool,
    is_per_mille: bool,
}

/// fn:format-number - Formats a number as a string using a picture string
///
/// # Arguments
/// * `$value` - The number to format
/// * `$picture` - Picture string (e.g., "#,##0.00")
/// * `$decimal-format-name` - Optional decimal format name
///
/// # Examples
/// * `format-number(1234.5, '#,##0.00')` → `"1,234.50"`
/// * `format-number(0.15, '0%')` → `"15%"`
/// * `format-number(-1234, '#,##0;(#,##0)')` → `"(1,234)"`
pub fn fn_format_number<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "format-number",
            "Expected 2 or 3 arguments",
        ));
    }

    let format_name = if args.len() == 3 {
        let name = args.remove(2).to_string_value();
        if name.is_empty() { None } else { Some(name) }
    } else {
        None
    };

    let picture = args.remove(1).to_string_value();

    let value_seq = args.remove(0);
    if value_seq.is_empty() {
        return Ok(XdmValue::from_string(String::new()));
    }

    let value = value_seq.to_double();

    let format = lookup_decimal_format(ctx, format_name.as_deref());

    let result = format_number_with_picture(value, &picture, &format)?;
    Ok(XdmValue::from_string(result))
}

fn lookup_decimal_format<'a, N: DataSourceNode<'a> + Clone + 'a>(
    ctx: &EvaluationContext<'a, '_, N>,
    name: Option<&str>,
) -> DecimalFormat {
    let var_name = match name {
        Some(n) => format!("::decimal-format:{}", n),
        None => "::decimal-format:".to_string(),
    };

    if let Some(val) = ctx.variables.get(&var_name) {
        let s = val.to_string_value();
        if let Some(df) = parse_decimal_format_from_string(&s) {
            return df;
        }
    }

    DecimalFormat::default()
}

fn parse_decimal_format_from_string(s: &str) -> Option<DecimalFormat> {
    let mut df = DecimalFormat::default();
    for part in s.split('\x1F') {
        if let Some((key, val)) = part.split_once('=') {
            match key {
                "ds" => df.decimal_separator = val.chars().next().unwrap_or('.'),
                "gs" => df.grouping_separator = val.chars().next().unwrap_or(','),
                "ms" => df.minus_sign = val.chars().next().unwrap_or('-'),
                "pc" => df.percent = val.chars().next().unwrap_or('%'),
                "pm" => df.per_mille = val.chars().next().unwrap_or('‰'),
                "zd" => df.zero_digit = val.chars().next().unwrap_or('0'),
                "dg" => df.digit = val.chars().next().unwrap_or('#'),
                "ps" => df.pattern_separator = val.chars().next().unwrap_or(';'),
                "inf" => df.infinity = val.to_string(),
                "nan" => df.nan = val.to_string(),
                _ => {}
            }
        }
    }
    Some(df)
}

fn format_number_with_picture(
    value: f64,
    picture: &str,
    format: &DecimalFormat,
) -> Result<String, XPath31Error> {
    // Handle special values
    if value.is_nan() {
        return Ok(format.nan.clone());
    }
    if value.is_infinite() {
        return Ok(if value.is_sign_negative() {
            format!("{}{}", format.minus_sign, format.infinity)
        } else {
            format.infinity.clone()
        });
    }

    // Split picture into positive and negative sub-pictures
    let patterns: Vec<&str> = picture.split(format.pattern_separator).collect();
    let (active_pattern, is_negative) = if value < 0.0 && patterns.len() > 1 {
        (patterns[1], true)
    } else {
        (patterns[0], value < 0.0)
    };

    let parsed = parse_picture(active_pattern, format)?;
    let abs_value = value.abs();

    // Apply percent/per-mille scaling
    let scaled_value = if parsed.is_percent {
        abs_value * 100.0
    } else if parsed.is_per_mille {
        abs_value * 1000.0
    } else {
        abs_value
    };

    // Format the number
    let formatted = format_scaled_number(scaled_value, &parsed, format);

    // Apply prefix/suffix and minus sign
    let mut result = String::new();

    // Add minus sign if negative and using primary pattern
    if is_negative && patterns.len() == 1 {
        result.push(format.minus_sign);
    }

    result.push_str(&parsed.prefix);
    result.push_str(&formatted);
    result.push_str(&parsed.suffix);

    Ok(result)
}

fn parse_picture(pattern: &str, format: &DecimalFormat) -> Result<NumberPicture, XPath31Error> {
    let mut pic = NumberPicture::default();
    let mut in_integer = true;
    let mut after_decimal = false;
    let mut current_group_size = 0;
    let mut seen_digit = false;
    let mut collecting_prefix = true;
    let mut collecting_suffix = false;

    for ch in pattern.chars() {
        if ch == format.digit || ch == format.zero_digit {
            collecting_prefix = false;
            seen_digit = true;

            if in_integer {
                if ch == format.zero_digit {
                    pic.integer_min_digits += 1;
                }
                pic.integer_max_digits += 1;
                current_group_size += 1;
            } else {
                if ch == format.zero_digit {
                    pic.fraction_min_digits += 1;
                }
                pic.fraction_max_digits += 1;
            }
        } else if ch == format.decimal_separator {
            collecting_prefix = false;
            in_integer = false;
            after_decimal = true;
            if pic.uses_grouping && current_group_size > 0 {
                pic.grouping_positions.push(current_group_size);
            }
        } else if ch == format.grouping_separator {
            if !after_decimal && seen_digit {
                pic.uses_grouping = true;
                pic.grouping_positions.push(current_group_size);
                current_group_size = 0;
            }
        } else if ch == format.percent {
            pic.is_percent = true;
            if collecting_prefix && !seen_digit {
                pic.prefix.push(ch);
            } else {
                collecting_suffix = true;
                pic.suffix.push(ch);
            }
        } else if ch == format.per_mille {
            pic.is_per_mille = true;
            if collecting_prefix && !seen_digit {
                pic.prefix.push(ch);
            } else {
                collecting_suffix = true;
                pic.suffix.push(ch);
            }
        } else if ch == format.minus_sign {
            if collecting_prefix && !seen_digit {
                pic.prefix.push(ch);
            } else {
                pic.suffix.push(ch);
            }
        } else if collecting_prefix && !seen_digit {
            // Any other character before digits is a prefix
            pic.prefix.push(ch);
        } else if seen_digit || collecting_suffix {
            // Any character after digits is a suffix
            collecting_suffix = true;
            pic.suffix.push(ch);
        }
    }

    // Record final group size if we haven't seen a decimal separator
    if pic.uses_grouping && in_integer && current_group_size > 0 {
        pic.grouping_positions.push(current_group_size);
    }

    // Ensure at least one digit position
    if pic.integer_max_digits == 0 && pic.fraction_max_digits == 0 {
        pic.integer_min_digits = 1;
        pic.integer_max_digits = 1;
    }

    Ok(pic)
}

fn format_scaled_number(value: f64, pic: &NumberPicture, format: &DecimalFormat) -> String {
    // Round to the required number of decimal places
    let rounded = if pic.fraction_max_digits > 0 {
        let factor = 10_f64.powi(pic.fraction_max_digits as i32);
        (value * factor).round() / factor
    } else {
        value.round()
    };

    // Split into integer and fraction parts
    let integer_part = rounded.trunc() as u64;
    let fraction_part = ((rounded - rounded.trunc()) * 10_f64.powi(pic.fraction_max_digits as i32))
        .round()
        .abs() as u64;

    // Format integer part
    let mut int_str = integer_part.to_string();

    // Pad with zeros if needed
    while int_str.len() < pic.integer_min_digits {
        int_str.insert(0, format.zero_digit);
    }

    // Apply grouping separators
    if pic.uses_grouping && !pic.grouping_positions.is_empty() {
        int_str = apply_grouping(&int_str, &pic.grouping_positions, format.grouping_separator);
    }

    // Format fraction part
    let mut result = int_str;

    if pic.fraction_max_digits > 0 {
        result.push(format.decimal_separator);

        let mut frac_str = format!(
            "{:0>width$}",
            fraction_part,
            width = pic.fraction_max_digits
        );

        // Trim trailing zeros down to minimum
        while frac_str.len() > pic.fraction_min_digits && frac_str.ends_with('0') {
            frac_str.pop();
        }

        result.push_str(&frac_str);
    } else if pic.fraction_min_digits > 0 {
        result.push(format.decimal_separator);
        for _ in 0..pic.fraction_min_digits {
            result.push(format.zero_digit);
        }
    }

    result
}

fn apply_grouping(s: &str, positions: &[usize], separator: char) -> String {
    if positions.is_empty() || s.is_empty() {
        return s.to_string();
    }

    let group_size = *positions.last().unwrap_or(&3);
    if group_size == 0 {
        return s.to_string();
    }

    let chars: Vec<char> = s.chars().collect();
    let mut result = String::new();
    let len = chars.len();

    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(group_size) {
            result.push(separator);
        }
        result.push(*ch);
    }

    result
}

pub fn fn_format_integer<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() < 2 || args.len() > 3 {
        return Err(XPath31Error::function(
            "format-integer",
            "Expected 2 or 3 arguments",
        ));
    }

    if args[0].is_empty() {
        return Ok(XdmValue::from_string(String::new()));
    }

    let value = args[0].to_double() as i64;
    let picture = args[1].to_string_value();

    let result = format_integer_with_picture(value, &picture)?;
    Ok(XdmValue::from_string(result))
}

fn format_integer_with_picture(value: i64, picture: &str) -> Result<String, XPath31Error> {
    let is_negative = value < 0;
    let abs_value = value.abs();

    let (prefix, format, suffix) = parse_integer_picture(picture);

    let formatted = match format.as_str() {
        "1" | "" => abs_value.to_string(),
        "01" => format!("{:02}", abs_value),
        "001" => format!("{:03}", abs_value),
        "0001" => format!("{:04}", abs_value),
        "a" => to_alphabetic(abs_value as usize, false),
        "A" => to_alphabetic(abs_value as usize, true),
        "i" => to_roman(abs_value as usize).to_lowercase(),
        "I" => to_roman(abs_value as usize),
        "w" => to_words(abs_value).to_lowercase(),
        "W" => to_words(abs_value).to_uppercase(),
        "Ww" => to_words_title_case(abs_value),
        _ if format.ends_with('o') || format.ends_with('c') => {
            let base = format.trim_end_matches(['o', 'c']);
            if format.ends_with('o') {
                format_ordinal(abs_value, base)
            } else {
                abs_value.to_string()
            }
        }
        _ => abs_value.to_string(),
    };

    let mut result = String::new();
    if is_negative {
        result.push('-');
    }
    result.push_str(&prefix);
    result.push_str(&formatted);
    result.push_str(&suffix);

    Ok(result)
}

fn parse_integer_picture(picture: &str) -> (String, String, String) {
    let chars: Vec<char> = picture.chars().collect();
    let mut prefix = String::new();
    let mut format = String::new();
    let mut suffix = String::new();

    let mut in_format = false;
    let mut done_format = false;

    for ch in chars {
        if !in_format && !done_format {
            if ch.is_ascii_alphanumeric() {
                in_format = true;
                format.push(ch);
            } else {
                prefix.push(ch);
            }
        } else if in_format {
            if ch.is_ascii_alphanumeric() {
                format.push(ch);
            } else {
                done_format = true;
                in_format = false;
                suffix.push(ch);
            }
        } else {
            suffix.push(ch);
        }
    }

    (prefix, format, suffix)
}

fn to_alphabetic(n: usize, uppercase: bool) -> String {
    if n == 0 {
        return String::new();
    }

    let mut result = String::new();
    let mut num = n;

    while num > 0 {
        num -= 1;
        let ch = if uppercase {
            (b'A' + (num % 26) as u8) as char
        } else {
            (b'a' + (num % 26) as u8) as char
        };
        result.insert(0, ch);
        num /= 26;
    }

    result
}

fn to_roman(n: usize) -> String {
    if n == 0 || n > 3999 {
        return n.to_string();
    }

    const ROMAN: [(usize, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];

    let mut result = String::new();
    let mut num = n;

    for (value, symbol) in ROMAN.iter() {
        while num >= *value {
            result.push_str(symbol);
            num -= *value;
        }
    }

    result
}

fn to_words(n: i64) -> String {
    if n == 0 {
        return "zero".to_string();
    }

    const ONES: [&str; 20] = [
        "",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];

    const TENS: [&str; 10] = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];

    fn convert_chunk(n: i64) -> String {
        if n == 0 {
            String::new()
        } else if n < 20 {
            ONES[n as usize].to_string()
        } else if n < 100 {
            let tens = TENS[(n / 10) as usize];
            let ones = ONES[(n % 10) as usize];
            if ones.is_empty() {
                tens.to_string()
            } else {
                format!("{}-{}", tens, ones)
            }
        } else {
            let hundreds = ONES[(n / 100) as usize];
            let remainder = n % 100;
            if remainder == 0 {
                format!("{} hundred", hundreds)
            } else {
                format!("{} hundred {}", hundreds, convert_chunk(remainder))
            }
        }
    }

    let mut parts = Vec::new();
    let mut num = n;

    const SCALES: [(i64, &str); 4] = [
        (1_000_000_000_000, "trillion"),
        (1_000_000_000, "billion"),
        (1_000_000, "million"),
        (1_000, "thousand"),
    ];

    for (scale, name) in SCALES.iter() {
        if num >= *scale {
            let count = num / scale;
            parts.push(format!("{} {}", convert_chunk(count), name));
            num %= scale;
        }
    }

    if num > 0 {
        parts.push(convert_chunk(num));
    }

    parts.join(" ")
}

fn to_words_title_case(n: i64) -> String {
    let words = to_words(n);
    words
        .split(' ')
        .map(|w| {
            if w.contains('-') {
                w.split('-')
                    .map(|part| {
                        let mut c = part.chars();
                        match c.next() {
                            None => String::new(),
                            Some(first) => first.to_uppercase().chain(c).collect(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("-")
            } else {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(c).collect(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_ordinal(n: i64, base_format: &str) -> String {
    let formatted = match base_format {
        "1" | "" => n.to_string(),
        "01" => format!("{:02}", n),
        "w" => {
            let ordinal_words = to_ordinal_words(n);
            return ordinal_words.to_lowercase();
        }
        "W" => {
            let ordinal_words = to_ordinal_words(n);
            return ordinal_words.to_uppercase();
        }
        "Ww" => return to_ordinal_words(n),
        _ => n.to_string(),
    };

    format!("{}{}", formatted, ordinal_suffix(n))
}

fn ordinal_suffix(n: i64) -> &'static str {
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

fn to_ordinal_words(n: i64) -> String {
    if n == 0 {
        return "zeroth".to_string();
    }

    const ORDINAL_ONES: [&str; 20] = [
        "",
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "eighth",
        "ninth",
        "tenth",
        "eleventh",
        "twelfth",
        "thirteenth",
        "fourteenth",
        "fifteenth",
        "sixteenth",
        "seventeenth",
        "eighteenth",
        "nineteenth",
    ];

    const ORDINAL_TENS: [&str; 10] = [
        "",
        "",
        "twentieth",
        "thirtieth",
        "fortieth",
        "fiftieth",
        "sixtieth",
        "seventieth",
        "eightieth",
        "ninetieth",
    ];

    if n < 20 {
        return ORDINAL_ONES[n as usize].to_string();
    }

    if n < 100 {
        if n % 10 == 0 {
            return ORDINAL_TENS[(n / 10) as usize].to_string();
        }

        const TENS: [&str; 10] = [
            "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
        ];
        return format!(
            "{}-{}",
            TENS[(n / 10) as usize],
            ORDINAL_ONES[(n % 10) as usize]
        );
    }

    let words = to_words(n);
    if let Some(last_space) = words.rfind(' ') {
        let (prefix, last_word) = words.split_at(last_space);
        let last_word = last_word.trim();
        if let Some(ordinal) = word_to_ordinal(last_word) {
            return format!("{} {}", prefix, ordinal);
        }
    }
    format!("{}th", words)
}

fn word_to_ordinal(word: &str) -> Option<String> {
    let ordinal = match word {
        "one" => "first",
        "two" => "second",
        "three" => "third",
        "four" => "fourth",
        "five" => "fifth",
        "six" => "sixth",
        "seven" => "seventh",
        "eight" => "eighth",
        "nine" => "ninth",
        "ten" => "tenth",
        "eleven" => "eleventh",
        "twelve" => "twelfth",
        "twenty" => "twentieth",
        "thirty" => "thirtieth",
        "forty" => "fortieth",
        "fifty" => "fiftieth",
        "sixty" => "sixtieth",
        "seventy" => "seventieth",
        "eighty" => "eightieth",
        "ninety" => "ninetieth",
        "hundred" => "hundredth",
        "thousand" => "thousandth",
        "million" => "millionth",
        "billion" => "billionth",
        "trillion" => "trillionth",
        _ => return None,
    };
    Some(ordinal.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EvaluationContext;
    use petty_xpath1::tests::MockNode;
    use std::collections::HashMap;

    fn create_empty_ctx() -> EvaluationContext<'static, 'static, MockNode<'static>> {
        static VARS: std::sync::OnceLock<HashMap<String, XdmValue<MockNode<'static>>>> =
            std::sync::OnceLock::new();
        let vars = VARS.get_or_init(HashMap::new);
        EvaluationContext::new(None, None, vars)
    }

    #[test]
    fn test_abs() {
        let result: XdmValue<()> = fn_abs(vec![XdmValue::from_double(-5.0)]).unwrap();
        assert_eq!(result.to_double(), 5.0);

        let result: XdmValue<()> = fn_abs(vec![XdmValue::from_double(5.0)]).unwrap();
        assert_eq!(result.to_double(), 5.0);
    }

    #[test]
    fn test_ceiling_floor() {
        let ceil: XdmValue<()> = fn_ceiling(vec![XdmValue::from_double(3.2)]).unwrap();
        assert_eq!(ceil.to_double(), 4.0);

        let floor: XdmValue<()> = fn_floor(vec![XdmValue::from_double(3.8)]).unwrap();
        assert_eq!(floor.to_double(), 3.0);
    }

    #[test]
    fn test_round() {
        let result: XdmValue<()> = fn_round(vec![XdmValue::from_double(2.5)]).unwrap();
        assert_eq!(result.to_double(), 3.0);

        let result: XdmValue<()> = fn_round(vec![XdmValue::from_double(2.4)]).unwrap();
        assert_eq!(result.to_double(), 2.0);

        let result: XdmValue<()> = fn_round(vec![
            XdmValue::from_double(3.56789),
            XdmValue::from_integer(2),
        ])
        .unwrap();
        assert!((result.to_double() - 3.57).abs() < 0.001);
    }

    #[test]
    fn test_sum() {
        let seq: XdmValue<()> = XdmValue::from_items(vec![
            XdmItem::Atomic(AtomicValue::Integer(1)),
            XdmItem::Atomic(AtomicValue::Integer(2)),
            XdmItem::Atomic(AtomicValue::Integer(3)),
        ]);
        let result = fn_sum(vec![seq]).unwrap();
        assert_eq!(result.to_double(), 6.0);
    }

    #[test]
    fn test_avg() {
        let seq: XdmValue<()> = XdmValue::from_items(vec![
            XdmItem::Atomic(AtomicValue::Integer(2)),
            XdmItem::Atomic(AtomicValue::Integer(4)),
            XdmItem::Atomic(AtomicValue::Integer(6)),
        ]);
        let result = fn_avg(vec![seq]).unwrap();
        assert_eq!(result.to_double(), 4.0);
    }

    #[test]
    fn test_min_max() {
        let seq: XdmValue<()> = XdmValue::from_items(vec![
            XdmItem::Atomic(AtomicValue::Integer(5)),
            XdmItem::Atomic(AtomicValue::Integer(2)),
            XdmItem::Atomic(AtomicValue::Integer(8)),
        ]);

        let min = fn_min(vec![seq.clone()]).unwrap();
        assert_eq!(min.to_double(), 2.0);

        let max = fn_max(vec![seq]).unwrap();
        assert_eq!(max.to_double(), 8.0);
    }

    #[test]
    fn test_format_number_basic() {
        let ctx = create_empty_ctx();
        let result = fn_format_number(
            vec![
                XdmValue::from_double(1234.5),
                XdmValue::from_string("#,##0.00".to_string()),
            ],
            &ctx,
        )
        .unwrap();
        assert_eq!(result.to_string_value(), "1,234.50");
    }

    #[test]
    fn test_format_number_no_decimals() {
        let ctx = create_empty_ctx();
        let result = fn_format_number(
            vec![
                XdmValue::from_double(1234.567),
                XdmValue::from_string("#,##0".to_string()),
            ],
            &ctx,
        )
        .unwrap();
        assert_eq!(result.to_string_value(), "1,235");
    }

    #[test]
    fn test_format_number_percent() {
        let ctx = create_empty_ctx();
        let result = fn_format_number(
            vec![
                XdmValue::from_double(0.15),
                XdmValue::from_string("0%".to_string()),
            ],
            &ctx,
        )
        .unwrap();
        assert_eq!(result.to_string_value(), "15%");
    }

    #[test]
    fn test_format_number_leading_zeros() {
        let ctx = create_empty_ctx();
        let result = fn_format_number(
            vec![
                XdmValue::from_double(5.0),
                XdmValue::from_string("000".to_string()),
            ],
            &ctx,
        )
        .unwrap();
        assert_eq!(result.to_string_value(), "005");
    }

    #[test]
    fn test_format_number_negative() {
        let ctx = create_empty_ctx();
        let result = fn_format_number(
            vec![
                XdmValue::from_double(-1234.0),
                XdmValue::from_string("#,##0".to_string()),
            ],
            &ctx,
        )
        .unwrap();
        assert_eq!(result.to_string_value(), "-1,234");
    }

    #[test]
    fn test_format_number_negative_pattern() {
        let ctx = create_empty_ctx();
        let result = fn_format_number(
            vec![
                XdmValue::from_double(-1234.0),
                XdmValue::from_string("#,##0;(#,##0)".to_string()),
            ],
            &ctx,
        )
        .unwrap();
        assert_eq!(result.to_string_value(), "(1,234)");
    }

    #[test]
    fn test_format_number_currency() {
        let ctx = create_empty_ctx();
        let result = fn_format_number(
            vec![
                XdmValue::from_double(1234.56),
                XdmValue::from_string("$#,##0.00".to_string()),
            ],
            &ctx,
        )
        .unwrap();
        assert_eq!(result.to_string_value(), "$1,234.56");
    }

    #[test]
    fn test_format_integer_cardinal() {
        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(123),
            XdmValue::from_string("1".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "123");
    }

    #[test]
    fn test_format_integer_ordinal() {
        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(1),
            XdmValue::from_string("1o".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "1st");

        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(2),
            XdmValue::from_string("1o".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "2nd");

        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(3),
            XdmValue::from_string("1o".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "3rd");
    }

    #[test]
    fn test_format_integer_roman() {
        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(2024),
            XdmValue::from_string("I".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "MMXXIV");

        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(4),
            XdmValue::from_string("i".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "iv");
    }

    #[test]
    fn test_format_integer_alphabetic() {
        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(1),
            XdmValue::from_string("a".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "a");

        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(26),
            XdmValue::from_string("A".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "Z");

        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(27),
            XdmValue::from_string("a".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "aa");
    }

    #[test]
    fn test_format_integer_words() {
        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(42),
            XdmValue::from_string("w".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "forty-two");

        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(123),
            XdmValue::from_string("W".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "ONE HUNDRED TWENTY-THREE");
    }

    #[test]
    fn test_format_integer_words_title() {
        let result: XdmValue<()> = fn_format_integer(vec![
            XdmValue::from_integer(42),
            XdmValue::from_string("Ww".to_string()),
        ])
        .unwrap();
        assert_eq!(result.to_string_value(), "Forty-Two");
    }
}
