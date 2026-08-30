#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::collapsible_if,
    clippy::format_collect,
    clippy::map_unwrap_or,
    clippy::needless_pass_by_value,
    clippy::redundant_closure_for_method_calls,
    clippy::single_match,
    clippy::single_match_else,
    clippy::struct_excessive_bools,
    clippy::too_many_lines,
    clippy::unchecked_time_subtraction,
    clippy::unnecessary_wraps,
    clippy::unreadable_literal,
    unsafe_code
)]

mod cli;
mod combo;
mod control;
mod daemon;
mod grab;
mod hid;
mod hw;
mod linux;
mod log;
mod mode;
mod pad;
mod paths;
mod poll;
mod scan;
mod slot;
mod steam;
mod steam_cfg;
mod sys;
#[cfg(test)]
mod test_env;
mod uhid;
mod urb;
mod usb;

fn main() {
    cli::main();
}
