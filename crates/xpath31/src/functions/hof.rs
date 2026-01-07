use std::collections::HashMap;

use crate::ast::{Expression, Param};
use crate::engine::{EvaluationContext, evaluate};
use crate::error::XPath31Error;
use crate::types::*;
use petty_xpath1::DataSourceNode;

pub fn invoke_function<'a, N: DataSourceNode<'a> + Clone + 'a>(
    func: &XdmFunction<N>,
    args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    match func {
        XdmFunction::Builtin { name, .. } => {
            super::call_function(&crate::ast::QName::new(name.clone()), args, ctx, local_vars)
        }
        XdmFunction::Inline {
            params,
            body,
            captured_variables,
        } => invoke_inline(params, body.as_ref(), captured_variables, args, ctx),
        XdmFunction::NamedRef {
            namespace,
            local_name,
            ..
        } => {
            let name = match namespace {
                Some(ns) => crate::ast::QName::with_prefix(ns.clone(), local_name.clone()),
                None => crate::ast::QName::new(local_name.clone()),
            };
            super::call_function(&name, args, ctx, local_vars)
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
            invoke_function(base, full_args, ctx, local_vars)
        }
    }
}

fn invoke_inline<'a, N: DataSourceNode<'a> + Clone + 'a>(
    params: &[Param],
    body: &Expression,
    captured: &[(String, XdmValue<N>)],
    args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
) -> Result<XdmValue<N>, XPath31Error> {
    let mut vars: HashMap<String, XdmValue<N>> = captured.iter().cloned().collect();

    for (i, param) in params.iter().enumerate() {
        if let Some(arg) = args.get(i) {
            vars.insert(param.name.clone(), arg.clone());
        } else {
            vars.insert(param.name.clone(), XdmValue::empty());
        }
    }

    evaluate(body, ctx, &vars)
}

pub fn fn_for_each<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("for-each", "Expected 2 arguments"));
    }

    let func_val = args.remove(1);
    let seq = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => return Err(XPath31Error::type_error("for-each requires a function")),
    };

    let mut results = Vec::new();
    for item in seq.items() {
        let result = invoke_function(
            &func,
            vec![XdmValue::from_item(item.clone())],
            ctx,
            local_vars,
        )?;
        results.extend(result.into_items());
    }

    Ok(XdmValue::from_items(results))
}

pub fn fn_filter<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("filter", "Expected 2 arguments"));
    }

    let func_val = args.remove(1);
    let seq = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => return Err(XPath31Error::type_error("filter requires a function")),
    };

    let mut results = Vec::new();
    for item in seq.items() {
        let result = invoke_function(
            &func,
            vec![XdmValue::from_item(item.clone())],
            ctx,
            local_vars,
        )?;
        if result.effective_boolean_value() {
            results.push(item.clone());
        }
    }

    Ok(XdmValue::from_items(results))
}

pub fn fn_fold_left<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 3 {
        return Err(XPath31Error::function("fold-left", "Expected 3 arguments"));
    }

    let func_val = args.remove(2);
    let zero = args.remove(1);
    let seq = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => return Err(XPath31Error::type_error("fold-left requires a function")),
    };

    let mut acc = zero;
    for item in seq.items() {
        acc = invoke_function(
            &func,
            vec![acc, XdmValue::from_item(item.clone())],
            ctx,
            local_vars,
        )?;
    }

    Ok(acc)
}

pub fn fn_fold_right<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 3 {
        return Err(XPath31Error::function("fold-right", "Expected 3 arguments"));
    }

    let func_val = args.remove(2);
    let zero = args.remove(1);
    let seq = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => return Err(XPath31Error::type_error("fold-right requires a function")),
    };

    let items: Vec<_> = seq.items().to_vec();
    let mut acc = zero;
    for item in items.into_iter().rev() {
        acc = invoke_function(&func, vec![XdmValue::from_item(item), acc], ctx, local_vars)?;
    }

    Ok(acc)
}

pub fn fn_for_each_pair<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 3 {
        return Err(XPath31Error::function(
            "for-each-pair",
            "Expected 3 arguments",
        ));
    }

    let func_val = args.remove(2);
    let seq2 = args.remove(1);
    let seq1 = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => {
            return Err(XPath31Error::type_error(
                "for-each-pair requires a function",
            ));
        }
    };

    let items1 = seq1.items();
    let items2 = seq2.items();
    let len = items1.len().min(items2.len());

    let mut results = Vec::new();
    for i in 0..len {
        let result = invoke_function(
            &func,
            vec![
                XdmValue::from_item(items1[i].clone()),
                XdmValue::from_item(items2[i].clone()),
            ],
            ctx,
            local_vars,
        )?;
        results.extend(result.into_items());
    }

    Ok(XdmValue::from_items(results))
}

pub fn fn_sort<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 3 {
        return Err(XPath31Error::function("sort", "Expected 1 to 3 arguments"));
    }

    let key_func = if args.len() == 3 {
        let f = args.remove(2);
        match f.first() {
            Some(XdmItem::Function(func)) => Some(func.clone()),
            _ => None,
        }
    } else {
        None
    };

    if args.len() >= 2 {
        args.remove(1);
    }

    let seq = args.remove(0);
    let mut items: Vec<XdmItem<N>> = seq.into_items();

    if let Some(func) = key_func {
        let mut keyed: Vec<(XdmItem<N>, XdmValue<N>)> = Vec::new();
        for item in items {
            let key = invoke_function(
                &func,
                vec![XdmValue::from_item(item.clone())],
                ctx,
                local_vars,
            )?;
            keyed.push((item, key));
        }
        keyed.sort_by(|a, b| compare_values(&a.1, &b.1));
        items = keyed.into_iter().map(|(item, _)| item).collect();
    } else {
        items.sort_by(|a, b| compare_items(a, b));
    }

    Ok(XdmValue::from_items(items))
}

fn compare_values<N: Clone>(a: &XdmValue<N>, b: &XdmValue<N>) -> std::cmp::Ordering {
    let a_str = a.to_string_value();
    let b_str = b.to_string_value();
    a_str.cmp(&b_str)
}

fn compare_items<N>(a: &XdmItem<N>, b: &XdmItem<N>) -> std::cmp::Ordering {
    match (a, b) {
        (XdmItem::Atomic(av), XdmItem::Atomic(bv)) => compare_atomics(av, bv),
        _ => std::cmp::Ordering::Equal,
    }
}

fn compare_atomics(a: &AtomicValue, b: &AtomicValue) -> std::cmp::Ordering {
    match (a, b) {
        (AtomicValue::String(s1), AtomicValue::String(s2)) => s1.cmp(s2),
        (AtomicValue::Integer(i1), AtomicValue::Integer(i2)) => i1.cmp(i2),
        (AtomicValue::Double(d1), AtomicValue::Double(d2)) => {
            d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal)
        }
        (AtomicValue::Integer(i), AtomicValue::Double(d)) => (*i as f64)
            .partial_cmp(d)
            .unwrap_or(std::cmp::Ordering::Equal),
        (AtomicValue::Double(d), AtomicValue::Integer(i)) => d
            .partial_cmp(&(*i as f64))
            .unwrap_or(std::cmp::Ordering::Equal),
        _ => a.to_string_value().cmp(&b.to_string_value()),
    }
}

pub fn fn_apply<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("apply", "Expected 2 arguments"));
    }

    let arr_val = args.remove(1);
    let func_val = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => return Err(XPath31Error::type_error("apply requires a function")),
    };

    let array = match arr_val.first() {
        Some(XdmItem::Array(a)) => a.clone(),
        _ => return Err(XPath31Error::type_error("apply requires an array")),
    };

    let func_args: Vec<XdmValue<N>> = array.members().to_vec();
    invoke_function(&func, func_args, ctx, local_vars)
}

pub fn map_for_each<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "map:for-each",
            "Expected 2 arguments",
        ));
    }

    let func_val = args.remove(1);
    let map_val = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => return Err(XPath31Error::type_error("map:for-each requires a function")),
    };

    let map = match map_val.first() {
        Some(XdmItem::Map(m)) => m.clone(),
        _ => return Err(XPath31Error::type_error("map:for-each requires a map")),
    };

    let mut results = Vec::new();
    for (key, value) in map.entries() {
        let result = invoke_function(
            &func,
            vec![XdmValue::from_atomic(key.clone()), value.clone()],
            ctx,
            local_vars,
        )?;
        results.extend(result.into_items());
    }

    Ok(XdmValue::from_items(results))
}

pub fn map_find<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    _ctx: &EvaluationContext<'a, '_, N>,
    _local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("map:find", "Expected 2 arguments"));
    }

    let key = args.remove(1);
    let input = args.remove(0);

    let key_atomic = match key.first() {
        Some(XdmItem::Atomic(a)) => a.clone(),
        _ => return Err(XPath31Error::type_error("map:find key must be atomic")),
    };

    fn find_in_item<N: Clone>(
        item: &XdmItem<N>,
        key: &AtomicValue,
        results: &mut Vec<XdmValue<N>>,
    ) {
        match item {
            XdmItem::Map(m) => {
                if let Some(v) = m.get(key) {
                    results.push(v.clone());
                }
                for (_, v) in m.entries() {
                    for sub_item in v.items() {
                        find_in_item(sub_item, key, results);
                    }
                }
            }
            XdmItem::Array(a) => {
                for member in a.members() {
                    for sub_item in member.items() {
                        find_in_item(sub_item, key, results);
                    }
                }
            }
            _ => {}
        }
    }

    let mut results = Vec::new();
    for item in input.items() {
        find_in_item(item, &key_atomic, &mut results);
    }

    let array = XdmArray::from_members(results);
    Ok(XdmValue::from_array(array))
}

pub fn array_for_each<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "array:for-each",
            "Expected 2 arguments",
        ));
    }

    let func_val = args.remove(1);
    let arr_val = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => {
            return Err(XPath31Error::type_error(
                "array:for-each requires a function",
            ));
        }
    };

    let array = match arr_val.first() {
        Some(XdmItem::Array(a)) => a.clone(),
        _ => return Err(XPath31Error::type_error("array:for-each requires an array")),
    };

    let mut results = Vec::new();
    for member in array.members() {
        let result = invoke_function(&func, vec![member.clone()], ctx, local_vars)?;
        results.push(result);
    }

    Ok(XdmValue::from_array(XdmArray::from_members(results)))
}

pub fn array_filter<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "array:filter",
            "Expected 2 arguments",
        ));
    }

    let func_val = args.remove(1);
    let arr_val = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => return Err(XPath31Error::type_error("array:filter requires a function")),
    };

    let array = match arr_val.first() {
        Some(XdmItem::Array(a)) => a.clone(),
        _ => return Err(XPath31Error::type_error("array:filter requires an array")),
    };

    let mut results = Vec::new();
    for member in array.members() {
        let result = invoke_function(&func, vec![member.clone()], ctx, local_vars)?;
        if result.effective_boolean_value() {
            results.push(member.clone());
        }
    }

    Ok(XdmValue::from_array(XdmArray::from_members(results)))
}

pub fn array_fold_left<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 3 {
        return Err(XPath31Error::function(
            "array:fold-left",
            "Expected 3 arguments",
        ));
    }

    let func_val = args.remove(2);
    let zero = args.remove(1);
    let arr_val = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => {
            return Err(XPath31Error::type_error(
                "array:fold-left requires a function",
            ));
        }
    };

    let array = match arr_val.first() {
        Some(XdmItem::Array(a)) => a.clone(),
        _ => {
            return Err(XPath31Error::type_error(
                "array:fold-left requires an array",
            ));
        }
    };

    let mut acc = zero;
    for member in array.members() {
        acc = invoke_function(&func, vec![acc, member.clone()], ctx, local_vars)?;
    }

    Ok(acc)
}

pub fn array_fold_right<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 3 {
        return Err(XPath31Error::function(
            "array:fold-right",
            "Expected 3 arguments",
        ));
    }

    let func_val = args.remove(2);
    let zero = args.remove(1);
    let arr_val = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => {
            return Err(XPath31Error::type_error(
                "array:fold-right requires a function",
            ));
        }
    };

    let array = match arr_val.first() {
        Some(XdmItem::Array(a)) => a.clone(),
        _ => {
            return Err(XPath31Error::type_error(
                "array:fold-right requires an array",
            ));
        }
    };

    let members: Vec<_> = array.members().to_vec();
    let mut acc = zero;
    for member in members.into_iter().rev() {
        acc = invoke_function(&func, vec![member, acc], ctx, local_vars)?;
    }

    Ok(acc)
}

pub fn array_sort<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.is_empty() || args.len() > 3 {
        return Err(XPath31Error::function(
            "array:sort",
            "Expected 1 to 3 arguments",
        ));
    }

    let key_func = if args.len() == 3 {
        let f = args.remove(2);
        match f.first() {
            Some(XdmItem::Function(func)) => Some(func.clone()),
            _ => None,
        }
    } else {
        None
    };

    if args.len() >= 2 {
        args.remove(1);
    }

    let arr_val = args.remove(0);

    let array = match arr_val.first() {
        Some(XdmItem::Array(a)) => a.clone(),
        _ => return Err(XPath31Error::type_error("array:sort requires an array")),
    };

    let mut members: Vec<XdmValue<N>> = array.members().to_vec();

    if let Some(func) = key_func {
        let mut keyed: Vec<(XdmValue<N>, XdmValue<N>)> = Vec::new();
        for member in members {
            let key = invoke_function(&func, vec![member.clone()], ctx, local_vars)?;
            keyed.push((member, key));
        }
        keyed.sort_by(|a, b| compare_values(&a.1, &b.1));
        members = keyed.into_iter().map(|(m, _)| m).collect();
    } else {
        members.sort_by(|a, b| compare_values(a, b));
    }

    Ok(XdmValue::from_array(XdmArray::from_members(members)))
}

pub fn fn_function_lookup<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    _ctx: &EvaluationContext<'a, '_, N>,
    _local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function(
            "function-lookup",
            "Expected 2 arguments",
        ));
    }

    let arity = args.remove(1).to_double() as usize;
    let name_arg = args.remove(0);

    let (namespace, local_name) = match name_arg.first() {
        Some(XdmItem::Atomic(AtomicValue::QName {
            prefix,
            local,
            namespace,
        })) => {
            let ns = namespace.clone().or_else(|| prefix.clone());
            (ns, local.clone())
        }
        _ => {
            return Err(XPath31Error::type_error(
                "function-lookup requires a QName as first argument",
            ));
        }
    };

    let func = XdmFunction::NamedRef {
        namespace,
        local_name,
        arity,
    };

    Ok(XdmValue::from_function(func))
}

pub fn fn_function_name<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "function-name",
            "Expected 1 argument",
        ));
    }

    let func_arg = args.remove(0);
    match func_arg.first() {
        Some(XdmItem::Function(f)) => match f {
            XdmFunction::NamedRef {
                namespace,
                local_name,
                ..
            } => Ok(XdmValue::from_atomic(AtomicValue::QName {
                prefix: namespace.clone(),
                local: local_name.clone(),
                namespace: namespace.clone(),
            })),
            XdmFunction::Builtin { name, .. } => {
                let (prefix, local) = if let Some(colon_pos) = name.find(':') {
                    (
                        Some(name[..colon_pos].to_string()),
                        name[colon_pos + 1..].to_string(),
                    )
                } else {
                    (Some("fn".to_string()), name.clone())
                };
                Ok(XdmValue::from_atomic(AtomicValue::QName {
                    prefix: prefix.clone(),
                    local,
                    namespace: prefix,
                }))
            }
            XdmFunction::Inline { .. } | XdmFunction::Partial { .. } => Ok(XdmValue::empty()),
        },
        _ => Err(XPath31Error::type_error(
            "function-name requires a function argument",
        )),
    }
}

pub fn fn_function_arity<N: Clone>(
    mut args: Vec<XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function(
            "function-arity",
            "Expected 1 argument",
        ));
    }

    let func_arg = args.remove(0);
    match func_arg.first() {
        Some(XdmItem::Function(f)) => {
            let arity = match f {
                XdmFunction::NamedRef { arity, .. } => *arity,
                XdmFunction::Builtin { arity, .. } => *arity,
                XdmFunction::Inline { params, .. } => params.len(),
                XdmFunction::Partial { bound_args, .. } => {
                    bound_args.iter().filter(|a| a.is_none()).count()
                }
            };
            Ok(XdmValue::from_integer(arity as i64))
        }
        _ => Err(XPath31Error::type_error(
            "function-arity requires a function argument",
        )),
    }
}

pub fn array_for_each_pair<'a, N: DataSourceNode<'a> + Clone + 'a>(
    mut args: Vec<XdmValue<N>>,
    ctx: &EvaluationContext<'a, '_, N>,
    local_vars: &HashMap<String, XdmValue<N>>,
) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 3 {
        return Err(XPath31Error::function(
            "array:for-each-pair",
            "Expected 3 arguments",
        ));
    }

    let func_val = args.remove(2);
    let arr2_val = args.remove(1);
    let arr1_val = args.remove(0);

    let func = match func_val.first() {
        Some(XdmItem::Function(f)) => f.clone(),
        _ => {
            return Err(XPath31Error::type_error(
                "array:for-each-pair requires a function",
            ));
        }
    };

    let arr1 = match arr1_val.first() {
        Some(XdmItem::Array(a)) => a.clone(),
        _ => {
            return Err(XPath31Error::type_error(
                "array:for-each-pair requires an array as first argument",
            ));
        }
    };

    let arr2 = match arr2_val.first() {
        Some(XdmItem::Array(a)) => a.clone(),
        _ => {
            return Err(XPath31Error::type_error(
                "array:for-each-pair requires an array as second argument",
            ));
        }
    };

    let members1 = arr1.members();
    let members2 = arr2.members();
    let len = members1.len().min(members2.len());

    let mut results = Vec::new();
    for i in 0..len {
        let result = invoke_function(
            &func,
            vec![members1[i].clone(), members2[i].clone()],
            ctx,
            local_vars,
        )?;
        results.push(result);
    }

    Ok(XdmValue::from_array(XdmArray::from_members(results)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_expression;
    use petty_xpath1::tests::{MockNode, create_test_tree};

    fn make_ctx<'a>() -> (
        petty_xpath1::tests::MockTree<'a>,
        HashMap<String, XdmValue<MockNode<'a>>>,
    ) {
        (create_test_tree(), HashMap::new())
    }

    #[test]
    fn test_for_each() {
        let (tree, vars) = make_ctx();
        let _ = &tree;
        let ctx: EvaluationContext<'_, '_, MockNode<'_>> =
            EvaluationContext::new(None, None, &vars);

        let double_fn = XdmFunction::inline(
            vec![Param {
                name: "x".to_string(),
                type_decl: None,
            }],
            parse_expression("$x * 2").unwrap(),
            vec![],
        );

        let result = fn_for_each(
            vec![
                XdmValue::from_items(vec![
                    XdmItem::Atomic(AtomicValue::Integer(1)),
                    XdmItem::Atomic(AtomicValue::Integer(2)),
                    XdmItem::Atomic(AtomicValue::Integer(3)),
                ]),
                XdmValue::from_function(double_fn),
            ],
            &ctx,
            &HashMap::new(),
        )
        .unwrap();

        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter() {
        let (tree, vars) = make_ctx();
        let _ = &tree;
        let ctx: EvaluationContext<'_, '_, MockNode<'_>> =
            EvaluationContext::new(None, None, &vars);

        let predicate = XdmFunction::inline(
            vec![Param {
                name: "x".to_string(),
                type_decl: None,
            }],
            parse_expression("$x > 1").unwrap(),
            vec![],
        );

        let result = fn_filter(
            vec![
                XdmValue::from_items(vec![
                    XdmItem::Atomic(AtomicValue::Integer(1)),
                    XdmItem::Atomic(AtomicValue::Integer(2)),
                    XdmItem::Atomic(AtomicValue::Integer(3)),
                ]),
                XdmValue::from_function(predicate),
            ],
            &ctx,
            &HashMap::new(),
        )
        .unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_fold_left() {
        let (tree, vars) = make_ctx();
        let _ = &tree;
        let ctx: EvaluationContext<'_, '_, MockNode<'_>> =
            EvaluationContext::new(None, None, &vars);

        let sum_fn = XdmFunction::inline(
            vec![
                Param {
                    name: "acc".to_string(),
                    type_decl: None,
                },
                Param {
                    name: "x".to_string(),
                    type_decl: None,
                },
            ],
            parse_expression("$acc + $x").unwrap(),
            vec![],
        );

        let result = fn_fold_left(
            vec![
                XdmValue::from_items(vec![
                    XdmItem::Atomic(AtomicValue::Integer(1)),
                    XdmItem::Atomic(AtomicValue::Integer(2)),
                    XdmItem::Atomic(AtomicValue::Integer(3)),
                ]),
                XdmValue::from_integer(0),
                XdmValue::from_function(sum_fn),
            ],
            &ctx,
            &HashMap::new(),
        )
        .unwrap();

        assert_eq!(result.to_double(), 6.0);
    }
}
