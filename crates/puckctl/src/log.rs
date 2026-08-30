use std::io::Write;

pub fn logln(msg: impl AsRef<str>) {
    println!("{}", msg.as_ref());
    let _ = std::io::stdout().flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_a_line() {
        logln("coverage");
        logln(String::from("owned"));
    }
}
