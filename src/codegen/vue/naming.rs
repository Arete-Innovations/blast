pub fn singularize(table: &str) -> String {
    for suffix in ["sses", "shes", "ches", "xes", "zes"] {
        match table.strip_suffix(suffix) {
            Some(stem) => return format!("{}{}", stem, &suffix[..suffix.len() - 2]),
            None => continue,
        }
    }
    match table.strip_suffix("ies") {
        Some(stem) => format!("{}y", stem),
        None => match table.strip_suffix('s') {
            Some(stem) => stem.to_string(),
            None => table.to_string(),
        },
    }
}

pub fn pascal_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut upper_next = true;
    for ch in input.chars() {
        if ch == '_' || ch == '-' {
            upper_next = true;
            continue;
        }
        if upper_next {
            for u in ch.to_uppercase() {
                out.push(u);
            }
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn ts_object_key(name: &str) -> String {
    let leads_with_letter = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    let plain_ident = leads_with_letter
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    if plain_ident {
        name.to_string()
    } else {
        format!("'{}'", name.replace('\\', "\\\\").replace('\'', "\\'"))
    }
}
