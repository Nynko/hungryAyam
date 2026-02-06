use std::fmt::Display;

pub fn option_to_string<T: Display>(opt: Option<T>) -> Option<String> {
    opt.map(|v| v.to_string())
}
