use std::fmt;

use mlua::{
    AnyUserData, Lua, MetaMethod, MultiValue, Result, UserData, UserDataMethods, UserDataRegistry, Value,
};
use owo_colors::{AnsiColors, DynColor};

pub(crate) struct Style {
    text: String,
    style: owo_colors::Style,
}

impl UserData for Style {
    fn register(registry: &mut UserDataRegistry<Self>) {
        // Sets the color for the text
        registry.add_function("color", |_, (ud, color): (AnyUserData, String)| {
            let mut this = ud.borrow_mut::<Self>()?;
            this.style = this.style.color(str2color(&color));
            Ok(ud)
        });

        // Sets the background color for the text
        registry.add_function("on", |_, (ud, color): (AnyUserData, String)| {
            let mut this = ud.borrow_mut::<Self>()?;
            this.style = this.style.on_color(str2color(&color));
            Ok(ud)
        });

        registry.add_meta_method(MetaMethod::ToString, |_, this, ()| Ok(format!("{this}")));
    }
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.style.style(&self.text).fmt(f)
    }
}

pub(crate) fn style(_: &Lua, value: Value) -> Result<Style> {
    let text = value.to_string()?;
    Ok(Style {
        text,
        style: owo_colors::Style::new(),
    })
}

pub(crate) fn print(_: &Lua, values: MultiValue) -> Result<()> {
    let mut first = true;
    for value in values {
        if !first {
            print!(" ");
        }
        first = false;
        print!("{}", value.to_string()?);
    }
    Ok(())
}

pub(crate) fn println(lua: &Lua, values: MultiValue) -> Result<()> {
    print(lua, values)?;
    println!();
    Ok(())
}

fn str2color(s: &str) -> impl DynColor {
    match s.to_ascii_lowercase().as_str() {
        "black" => AnsiColors::Black,
        "red" => AnsiColors::Red,
        "green" => AnsiColors::Green,
        "yellow" => AnsiColors::Yellow,
        "blue" => AnsiColors::Blue,
        "magenta" => AnsiColors::Magenta,
        "cyan" => AnsiColors::Cyan,
        "white" => AnsiColors::White,
        "bright_black" => AnsiColors::BrightBlack,
        "bright_red" => AnsiColors::BrightRed,
        "bright_green" => AnsiColors::BrightGreen,
        "bright_yellow" => AnsiColors::BrightYellow,
        "bright_blue" => AnsiColors::BrightBlue,
        "bright_magenta" => AnsiColors::BrightMagenta,
        "bright_cyan" => AnsiColors::BrightCyan,
        "bright_white" => AnsiColors::BrightWhite,
        _ => AnsiColors::Default,
    }
}
