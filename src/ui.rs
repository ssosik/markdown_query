use crossbeam_channel::unbounded;
use cursive::backends::crossterm::crossterm::style::Stylize;
use cursive::views::{Dialog, LinearLayout};

pub fn run() {
    let mut siv = cursive::default();
    siv.load_toml(include_str!("../assets/theme.toml")).unwrap();

    let mut siv = siv.runner();
}
