mod boolean;
mod core;
pub mod datetime;
pub mod hof;
mod json;
mod math;
mod numeric;
mod regex;
mod sequence;
mod string;

use std::collections::HashMap;

use crate::ast::QName;
use crate::engine::{EvaluationContext, evaluate};
use crate::error::XPath31Error;
use crate::types::{XdmFunction, XdmItem, XdmValue};
use petty_xpath1::DataSourceNode;

pub fn call_function<'a, N: DataSourceNode<'a> + Clone + 'a>(
    name: &QName,
    args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    let full_name = name.to_string();
    let prefix = name.prefix.as_deref();
    let local = name.local_part.as_str();

    match (prefix, local) {
        (Some("fn") | None, "true") => boolean::fn_true(args),
        (Some("fn") | None, "false") => boolean::fn_false(args),
        (Some("fn") | None, "not") => boolean::fn_not(args),
        (Some("fn") | None, "boolean") => boolean::fn_boolean(args),

        (Some("fn") | None, "concat") => string::fn_concat(args),
        (Some("fn") | None, "string") => string::fn_string(args, ctx),
        (Some("fn") | None, "string-length") => string::fn_string_length(args, ctx),
        (Some("fn") | None, "substring") => string::fn_substring(args),
        (Some("fn") | None, "contains") => string::fn_contains(args),
        (Some("fn") | None, "starts-with") => string::fn_starts_with(args),
        (Some("fn") | None, "ends-with") => string::fn_ends_with(args),
        (Some("fn") | None, "upper-case") => string::fn_upper_case(args),
        (Some("fn") | None, "lower-case") => string::fn_lower_case(args),
        (Some("fn") | None, "normalize-space") => string::fn_normalize_space(args, ctx),
        (Some("fn") | None, "translate") => string::fn_translate(args),
        (Some("fn") | None, "replace") => string::fn_replace(args),
        (Some("fn") | None, "tokenize") => string::fn_tokenize(args),
        (Some("fn") | None, "string-join") => string::fn_string_join(args),
        (Some("fn") | None, "substring-before") => string::fn_substring_before(args),
        (Some("fn") | None, "substring-after") => string::fn_substring_after(args),
        (Some("fn") | None, "compare") => string::fn_compare(args),
        (Some("fn") | None, "codepoints-to-string") => string::fn_codepoints_to_string(args),
        (Some("fn") | None, "string-to-codepoints") => string::fn_string_to_codepoints(args),
        (Some("fn") | None, "encode-for-uri") => string::fn_encode_for_uri(args),
        (Some("fn") | None, "iri-to-uri") => string::fn_iri_to_uri(args),
        (Some("fn") | None, "normalize-unicode") => string::fn_normalize_unicode(args),
        (Some("fn") | None, "resolve-uri") => string::fn_resolve_uri(args),
        (Some("fn") | None, "base-uri") => string::fn_base_uri(args, ctx),
        (Some("fn") | None, "static-base-uri") => string::fn_static_base_uri(args),
        (Some("fn") | None, "contains-token") => string::fn_contains_token(args),
        (Some("fn") | None, "default-collation") => string::fn_default_collation(args),
        (Some("fn") | None, "default-language") => string::fn_default_language(args),
        (Some("fn") | None, "collation-key") => string::fn_collation_key(args),

        (Some("fn") | None, "abs") => numeric::fn_abs(args),
        (Some("fn") | None, "ceiling") => numeric::fn_ceiling(args),
        (Some("fn") | None, "floor") => numeric::fn_floor(args),
        (Some("fn") | None, "round") => numeric::fn_round(args),
        (Some("fn") | None, "round-half-to-even") => numeric::fn_round_half_to_even(args),
        (Some("fn") | None, "number") => numeric::fn_number(args, ctx),
        (Some("fn") | None, "sum") => numeric::fn_sum(args),
        (Some("fn") | None, "avg") => numeric::fn_avg(args),
        (Some("fn") | None, "min") => numeric::fn_min(args),
        (Some("fn") | None, "max") => numeric::fn_max(args),
        (Some("fn") | None, "format-number") => numeric::fn_format_number(args, ctx),
        (Some("fn") | None, "format-integer") => numeric::fn_format_integer(args),

        (Some("fn") | None, "count") => sequence::fn_count(args),
        (Some("fn") | None, "empty") => sequence::fn_empty(args),
        (Some("fn") | None, "exists") => sequence::fn_exists(args),
        (Some("fn") | None, "head") => sequence::fn_head(args),
        (Some("fn") | None, "tail") => sequence::fn_tail(args),
        (Some("fn") | None, "reverse") => sequence::fn_reverse(args),
        (Some("fn") | None, "subsequence") => sequence::fn_subsequence(args),
        (Some("fn") | None, "distinct-values") => sequence::fn_distinct_values(args),
        (Some("fn") | None, "insert-before") => sequence::fn_insert_before(args),
        (Some("fn") | None, "remove") => sequence::fn_remove(args),
        (Some("fn") | None, "deep-equal") => sequence::fn_deep_equal(args),
        (Some("fn") | None, "index-of") => sequence::fn_index_of(args),
        (Some("fn") | None, "zero-or-one") => sequence::fn_zero_or_one(args),
        (Some("fn") | None, "one-or-more") => sequence::fn_one_or_more(args),
        (Some("fn") | None, "exactly-one") => sequence::fn_exactly_one(args),
        (Some("fn") | None, "innermost") => sequence::fn_innermost(args),
        (Some("fn") | None, "outermost") => sequence::fn_outermost(args),

        (Some("fn") | None, "position") => core::fn_position(ctx),
        (Some("fn") | None, "last") => core::fn_last(ctx),
        (Some("fn") | None, "local-name") => core::fn_local_name(args, ctx),
        (Some("fn") | None, "namespace-uri") => core::fn_namespace_uri(args, ctx),
        (Some("fn") | None, "name") => core::fn_name(args, ctx),
        (Some("fn") | None, "root") => core::fn_root(args, ctx),
        (Some("fn") | None, "generate-id") => core::fn_generate_id(args, ctx),
        (Some("fn") | None, "error") => core::fn_error(args),
        (Some("fn") | None, "trace") => core::fn_trace(args),
        (Some("fn") | None, "data") => core::fn_data(args, ctx),
        (Some("fn") | None, "node-name") => core::fn_node_name(args, ctx),
        (Some("fn") | None, "nilled") => core::fn_nilled(args, ctx),
        (Some("fn") | None, "lang") => core::fn_lang(args, ctx),
        (Some("fn") | None, "in-scope-prefixes") => core::fn_in_scope_prefixes(args),
        (Some("fn") | None, "namespace-uri-for-prefix") => core::fn_namespace_uri_for_prefix(args),
        (Some("fn") | None, "QName") => core::fn_qname(args),
        (Some("fn") | None, "prefix-from-QName") => core::fn_prefix_from_qname(args),
        (Some("fn") | None, "local-name-from-QName") => core::fn_local_name_from_qname(args),
        (Some("fn") | None, "namespace-uri-from-QName") => core::fn_namespace_uri_from_qname(args),
        (Some("fn") | None, "resolve-QName") => core::fn_resolve_qname(args),
        (Some("fn") | None, "has-children") => core::fn_has_children(args, ctx),
        (Some("fn") | None, "id") => core::fn_id(args, ctx),
        (Some("fn") | None, "idref") => core::fn_idref(args, ctx),
        (Some("fn") | None, "element-with-id") => core::fn_element_with_id(args, ctx),
        (Some("fn") | None, "system-property") => core::fn_system_property(args),
        (Some("fn") | None, "environment-variable") => core::fn_environment_variable(args),
        (Some("fn") | None, "available-environment-variables") => {
            core::fn_available_environment_variables(args)
        }
        (Some("fn") | None, "random-number-generator") => core::fn_random_number_generator(args),
        (Some("fn") | None, "random-number-generator-permute") => {
            core::fn_random_number_generator_permute(args)
        }

        (Some("fn") | None, "current-grouping-key") => local_vars
            .get("::current-grouping-key")
            .cloned()
            .ok_or_else(|| XPath31Error::function(full_name.clone(), "Not in a grouping context")),
        (Some("fn") | None, "current-group") => local_vars
            .get("::current-group")
            .cloned()
            .ok_or_else(|| XPath31Error::function(full_name.clone(), "Not in a grouping context")),

        // XSLT 3.0 merge context functions
        (Some("fn") | None, "current-merge-key") => local_vars
            .get("::current-merge-key")
            .cloned()
            .ok_or_else(|| XPath31Error::function(full_name.clone(), "Not in a merge context")),
        (Some("fn") | None, "current-merge-group") => local_vars
            .get("::current-merge-group")
            .cloned()
            .ok_or_else(|| XPath31Error::function(full_name.clone(), "Not in a merge context")),
        (Some("fn") | None, "current-merge-source") => local_vars
            .get("::current-merge-source")
            .cloned()
            .ok_or_else(|| XPath31Error::function(full_name.clone(), "Not in a merge context")),

        (Some("fn") | None, "key") => {
            if args.len() != 2 {
                return Err(XPath31Error::function(
                    full_name,
                    "key() requires exactly 2 arguments (name, value)",
                ));
            }
            let key_name = args[0].to_string_value();
            let key_value = args[1].to_string_value();

            let index_var_name = format!("::key-index:{}", key_name);
            if let Some(index_map) = local_vars.get(&index_var_name)
                && let Some(XdmItem::Map(map)) = index_map.first()
            {
                let lookup_key = crate::types::AtomicValue::String(key_value);
                if let Some(result) = map.get(&lookup_key) {
                    return Ok(result.clone());
                }
            }
            Ok(XdmValue::empty())
        }

        (Some("fn") | None, "regex-group") => {
            if args.len() != 1 {
                return Err(XPath31Error::function(
                    full_name,
                    "regex-group() requires exactly 1 argument",
                ));
            }
            let group_num = args[0].to_string_value().parse::<usize>().unwrap_or(0);
            if group_num == 0 {
                return Ok(XdmValue::from_string(String::new()));
            }
            let var_name = format!("::regex-group{}", group_num);
            Ok(local_vars
                .get(&var_name)
                .cloned()
                .unwrap_or_else(|| XdmValue::from_string(String::new())))
        }

        (Some("fn") | None, "for-each") => hof::fn_for_each(args, ctx, local_vars),
        (Some("fn") | None, "filter") => hof::fn_filter(args, ctx, local_vars),
        (Some("fn") | None, "fold-left") => hof::fn_fold_left(args, ctx, local_vars),
        (Some("fn") | None, "fold-right") => hof::fn_fold_right(args, ctx, local_vars),
        (Some("fn") | None, "for-each-pair") => hof::fn_for_each_pair(args, ctx, local_vars),
        (Some("fn") | None, "sort") => hof::fn_sort(args, ctx, local_vars),
        (Some("fn") | None, "apply") => hof::fn_apply(args, ctx, local_vars),
        (Some("fn") | None, "function-lookup") => hof::fn_function_lookup(args, ctx, local_vars),
        (Some("fn") | None, "function-name") => hof::fn_function_name(args),
        (Some("fn") | None, "function-arity") => hof::fn_function_arity(args),

        (Some("map"), "size") => core::map_size(args),
        (Some("map"), "keys") => core::map_keys(args),
        (Some("map"), "contains") => core::map_contains(args),
        (Some("map"), "get") => core::map_get(args),
        (Some("map"), "put") => core::map_put(args),
        (Some("map"), "remove") => core::map_remove(args),
        (Some("map"), "entry") => core::map_entry(args),
        (Some("map"), "merge") => core::map_merge(args),
        (Some("map"), "for-each") => hof::map_for_each(args, ctx, local_vars),
        (Some("map"), "find") => hof::map_find(args, ctx, local_vars),

        (Some("array"), "size") => core::array_size(args),
        (Some("array"), "get") => core::array_get(args),
        (Some("array"), "put") => core::array_put(args),
        (Some("array"), "append") => core::array_append(args),
        (Some("array"), "head") => core::array_head(args),
        (Some("array"), "tail") => core::array_tail(args),
        (Some("array"), "reverse") => core::array_reverse(args),
        (Some("array"), "join") => core::array_join(args),
        (Some("array"), "subarray") => core::array_subarray(args),
        (Some("array"), "remove") => core::array_remove(args),
        (Some("array"), "insert-before") => core::array_insert_before(args),
        (Some("array"), "flatten") => core::array_flatten(args),
        (Some("array"), "for-each") => hof::array_for_each(args, ctx, local_vars),
        (Some("array"), "filter") => hof::array_filter(args, ctx, local_vars),
        (Some("array"), "fold-left") => hof::array_fold_left(args, ctx, local_vars),
        (Some("array"), "fold-right") => hof::array_fold_right(args, ctx, local_vars),
        (Some("array"), "sort") => hof::array_sort(args, ctx, local_vars),
        (Some("array"), "for-each-pair") => hof::array_for_each_pair(args, ctx, local_vars),

        (Some("fn") | None, "parse-json") => json::fn_parse_json(args),
        (Some("fn") | None, "json-doc") => json::fn_json_doc(args),
        (Some("fn") | None, "json-to-xml") => json::fn_json_to_xml(args),
        (Some("fn") | None, "xml-to-json") => json::fn_xml_to_json(args),

        (Some("math"), "pi") => math::math_pi(args),
        (Some("math"), "exp") => math::math_exp(args),
        (Some("math"), "exp10") => math::math_exp10(args),
        (Some("math"), "log") => math::math_log(args),
        (Some("math"), "log10") => math::math_log10(args),
        (Some("math"), "pow") => math::math_pow(args),
        (Some("math"), "sqrt") => math::math_sqrt(args),
        (Some("math"), "sin") => math::math_sin(args),
        (Some("math"), "cos") => math::math_cos(args),
        (Some("math"), "tan") => math::math_tan(args),
        (Some("math"), "asin") => math::math_asin(args),
        (Some("math"), "acos") => math::math_acos(args),
        (Some("math"), "atan") => math::math_atan(args),
        (Some("math"), "atan2") => math::math_atan2(args),

        (Some("fn") | None, "analyze-string") => regex::fn_analyze_string(args),
        (Some("fn") | None, "matches") => regex::fn_matches(args),

        (Some("fn") | None, "current-dateTime") => datetime::fn_current_datetime(args),
        (Some("fn") | None, "current-date") => datetime::fn_current_date(args),
        (Some("fn") | None, "current-time") => datetime::fn_current_time(args),
        (Some("fn") | None, "dateTime") => datetime::fn_datetime(args),
        (Some("fn") | None, "format-dateTime") => datetime::fn_format_datetime(args),
        (Some("fn") | None, "format-date") => datetime::fn_format_date(args),
        (Some("fn") | None, "format-time") => datetime::fn_format_time(args),
        (Some("fn") | None, "implicit-timezone") => datetime::fn_implicit_timezone(args),

        (Some("fn") | None, "year-from-dateTime") => datetime::fn_year_from_datetime(args),
        (Some("fn") | None, "month-from-dateTime") => datetime::fn_month_from_datetime(args),
        (Some("fn") | None, "day-from-dateTime") => datetime::fn_day_from_datetime(args),
        (Some("fn") | None, "hours-from-dateTime") => datetime::fn_hours_from_datetime(args),
        (Some("fn") | None, "minutes-from-dateTime") => datetime::fn_minutes_from_datetime(args),
        (Some("fn") | None, "seconds-from-dateTime") => datetime::fn_seconds_from_datetime(args),
        (Some("fn") | None, "timezone-from-dateTime") => datetime::fn_timezone_from_datetime(args),

        (Some("fn") | None, "year-from-date") => datetime::fn_year_from_date(args),
        (Some("fn") | None, "month-from-date") => datetime::fn_month_from_date(args),
        (Some("fn") | None, "day-from-date") => datetime::fn_day_from_date(args),
        (Some("fn") | None, "timezone-from-date") => datetime::fn_timezone_from_date(args),

        (Some("fn") | None, "hours-from-time") => datetime::fn_hours_from_time(args),
        (Some("fn") | None, "minutes-from-time") => datetime::fn_minutes_from_time(args),
        (Some("fn") | None, "seconds-from-time") => datetime::fn_seconds_from_time(args),
        (Some("fn") | None, "timezone-from-time") => datetime::fn_timezone_from_time(args),

        (Some("fn") | None, "years-from-duration") => datetime::fn_years_from_duration(args),
        (Some("fn") | None, "months-from-duration") => datetime::fn_months_from_duration(args),
        (Some("fn") | None, "days-from-duration") => datetime::fn_days_from_duration(args),
        (Some("fn") | None, "hours-from-duration") => datetime::fn_hours_from_duration(args),
        (Some("fn") | None, "minutes-from-duration") => datetime::fn_minutes_from_duration(args),
        (Some("fn") | None, "seconds-from-duration") => datetime::fn_seconds_from_duration(args),

        (Some("fn") | None, "adjust-dateTime-to-timezone") => {
            datetime::fn_adjust_datetime_to_timezone(args)
        }
        (Some("fn") | None, "adjust-date-to-timezone") => {
            datetime::fn_adjust_date_to_timezone(args)
        }
        (Some("fn") | None, "adjust-time-to-timezone") => {
            datetime::fn_adjust_time_to_timezone(args)
        }

        (Some("fn") | None, "subtract-dates") => datetime::fn_subtract_dates(args),
        (Some("fn") | None, "subtract-dateTimes") => datetime::fn_subtract_datetimes(args),
        (Some("fn") | None, "subtract-times") => datetime::fn_subtract_times(args),
        (Some("fn") | None, "add-dayTimeDuration-to-date") => {
            datetime::fn_add_daytimeduration_to_date(args)
        }
        (Some("fn") | None, "add-dayTimeDuration-to-dateTime") => {
            datetime::fn_add_daytimeduration_to_datetime(args)
        }
        (Some("fn") | None, "add-dayTimeDuration-to-time") => {
            datetime::fn_add_daytimeduration_to_time(args)
        }
        (Some("fn") | None, "add-yearMonthDuration-to-date") => {
            datetime::fn_add_yearmonth_duration_to_date(args)
        }
        (Some("fn") | None, "add-yearMonthDuration-to-dateTime") => {
            datetime::fn_add_yearmonth_duration_to_datetime(args)
        }
        (Some("fn") | None, "parse-ietf-date") => datetime::fn_parse_ietf_date(args),

        (Some("fn") | None, "doc") => core::fn_doc(args),
        (Some("fn") | None, "doc-available") => core::fn_doc_available(args),
        (Some("fn") | None, "collection") => core::fn_collection(args),
        (Some("fn") | None, "uri-collection") => core::fn_uri_collection(args),
        (Some("fn") | None, "unparsed-text") => core::fn_unparsed_text(args),
        (Some("fn") | None, "unparsed-text-available") => core::fn_unparsed_text_available(args),
        (Some("fn") | None, "unparsed-text-lines") => core::fn_unparsed_text_lines(args),
        (Some("fn") | None, "parse-xml") => core::fn_parse_xml(args),
        (Some("fn") | None, "parse-xml-fragment") => core::fn_parse_xml_fragment(args),
        (Some("fn") | None, "serialize") => core::fn_serialize(args),
        (Some("fn") | None, "path") => core::fn_path(args, ctx),

        _ => Err(XPath31Error::function(full_name, "Unknown function")),
    }
}

pub fn call_xdm_function<'a, N: DataSourceNode<'a> + Clone + 'a>(
    func: &XdmFunction<N>,
    args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    match func {
        XdmFunction::NamedRef {
            namespace,
            local_name,
            arity,
        } => {
            if args.len() != *arity {
                return Err(XPath31Error::type_error(format!(
                    "Function expects {} arguments, got {}",
                    arity,
                    args.len()
                )));
            }
            let name = QName {
                prefix: namespace.clone(),
                local_part: local_name.clone(),
            };
            call_function(&name, args, ctx, local_vars)
        }
        XdmFunction::Inline {
            params,
            body,
            captured_variables,
        } => {
            if args.len() != params.len() {
                return Err(XPath31Error::type_error(format!(
                    "Function expects {} arguments, got {}",
                    params.len(),
                    args.len()
                )));
            }

            let mut new_vars = local_vars.clone();

            for (name, val) in captured_variables {
                new_vars.insert(name.clone(), val.clone());
            }

            for (param, arg) in params.iter().zip(args.into_iter()) {
                new_vars.insert(param.name.clone(), arg);
            }

            evaluate(body, ctx, &new_vars)
        }
        XdmFunction::Builtin { name, arity } => {
            if args.len() != *arity {
                return Err(XPath31Error::type_error(format!(
                    "Function expects {} arguments, got {}",
                    arity,
                    args.len()
                )));
            }
            let qname = QName::new(name);
            call_function(&qname, args, ctx, local_vars)
        }
        XdmFunction::Partial { base, bound_args } => {
            let mut full_args = Vec::new();
            let mut arg_iter = args.into_iter();
            for bound in bound_args {
                match bound {
                    Some(v) => full_args.push(v.clone()),
                    None => {
                        if let Some(a) = arg_iter.next() {
                            full_args.push(a);
                        }
                    }
                }
            }
            full_args.extend(arg_iter);
            call_xdm_function(base, full_args, ctx, local_vars)
        }
    }
}
