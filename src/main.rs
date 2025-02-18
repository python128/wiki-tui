extern crate anyhow;
extern crate lazy_static;
extern crate log;

use anyhow::*;
use core::panic;
use cursive::align::HAlign;
use cursive::theme::*;
use cursive::traits::*;
use cursive::view::Resizable;
use cursive::views::*;
use cursive::Cursive;
use std::fs;
use std::io::Write;

pub mod cli;
pub mod config;
pub mod error;
pub mod logging;
pub mod ui;
pub mod wiki;

pub const LOGO: &str = "
 _      __   (_)   / /__   (_)         / /_  __  __   (_) 
| | /| / /  / /   / //_/  / /  ______ / __/ / / / /  / /  
| |/ |/ /  / /   / ,<    / /  /_____// /_  / /_/ /  / /   
|__/|__/  /_/   /_/|_|  /_/          \\__/  \\__,_/  /_/    
";

fn main() {
    let mut data = std::collections::HashMap::new();
    data.insert("%NAME%", env!("CARGO_PKG_NAME"));
    data.insert("%GITHUB%", env!("CARGO_PKG_REPOSITORY"));

    error::create_hook(Some(data), |path, data| {
        if let Some(path) = path {
            let mut fs = fs::File::create(path).unwrap();
            fs.write_all(data.as_bytes())
                .expect("Unable to generate report");
        };
    });

    let wiki = match initialize() {
        Ok(wiki) => wiki,
        Err(error) => {
            panic!("Something happend during initialization:\n{:?}", error);
        }
    };

    start_application(wiki);
}

fn initialize() -> Result<wiki::WikiApi> {
    println!("{}", LOGO);

    // create and initialize the logger
    logging::Logger::new().initialize();

    // Create the wiki struct, used for interaction with the wikipedia website/api
    Ok(wiki::WikiApi::new())
}

fn start_application(wiki: wiki::WikiApi) {
    let mut siv = cursive::default();
    siv.add_global_callback('q', Cursive::quit);
    siv.set_user_data(wiki);

    // get and apply the color theme
    let theme = Theme {
        palette: get_color_palette(),
        ..Default::default()
    };
    siv.set_theme(theme);

    // Create the views
    let search_bar = EditView::new()
        .on_submit(|s, q| match ui::search::on_search(s, q.to_string()) {
            Ok(_) => (),
            Err(error) => {log::error!("{:?}", error); panic!("Something happened while searching. Please check your logs for further information")},
        })
        .style({
            if let Some(search_theme) = &config::CONFIG.theme.search_bar {
                if search_theme.background == search_theme.secondary {
                    ColorStyle::new(search_theme.background, search_theme.text)
                } else { ColorStyle::secondary() }
            } else { ColorStyle::secondary() }
        })
        .with_name("search_bar")
        .full_width();

    let search_layout = change_theme!(
        config::CONFIG.theme.search_bar,
        Dialog::around(LinearLayout::horizontal().child(search_bar))
            .title("Search")
            .title_position(cursive::align::HAlign::Left)
    );

    let logo_view = TextView::new(LOGO)
        .h_align(HAlign::Center)
        .with_name("logo_view")
        .full_screen();

    let article_layout = LinearLayout::horizontal()
        .child(Dialog::around(logo_view))
        .with_name("article_layout");

    // Add a fullscreen layer, containing the search bar and the article view
    siv.add_fullscreen_layer(
        Dialog::around(
            LinearLayout::vertical()
                .child(search_layout)
                .child(article_layout),
        )
        .title("wiki-tui")
        .button("Quit", Cursive::quit)
        .full_screen(),
    );

    // Start the application
    let argument_callback = handle_arguments();
    if let Err(error) = siv.cb_sink().send(argument_callback) {
        log::error!("{:?}", error);
    }

    let siv_box = std::sync::Mutex::new(siv);
    if let Err(err) = std::panic::catch_unwind(|| {
        siv_box.lock().unwrap().run();
    }) {
        panic!("{}", panic_message::panic_message(&err));
    }
}

fn handle_arguments() -> Box<dyn FnOnce(&mut Cursive) + Send> {
    if let Some(search_query) = config::CONFIG.get_args().search_query.as_ref() {
        log::info!("Searching for the article: {}", search_query);
        return Box::new(move |siv: &mut Cursive| {
            if let Err(error) = ui::search::on_search(siv, search_query.to_string()) {
                log::error!("{:?}", error);
                panic!("Something happened while searching. Please check your logs for further information");
            };
        });
    } else if let Some(article_id) = config::CONFIG.get_args().article_id {
        log::info!("Opening the article: {}", article_id);
        return Box::new(move |siv: &mut Cursive| {
            ui::article::on_article_submit(siv, &article_id.into());
        });
    }

    Box::new(|_: &mut Cursive| {})
}

fn get_color_palette() -> Palette {
    let mut custom_palette = Palette::default();

    custom_palette.set_color("View", config::CONFIG.theme.background);
    custom_palette.set_color("Primary", config::CONFIG.theme.text);
    custom_palette.set_color("TitlePrimary", config::CONFIG.theme.title);
    custom_palette.set_color("Highlight", config::CONFIG.theme.highlight);
    custom_palette.set_color("HighlightInactive", config::CONFIG.theme.highlight_inactive);
    custom_palette.set_color("HighlightText", config::CONFIG.theme.highlight_text);

    custom_palette
}

fn remove_view_from_article_layout(siv: &mut Cursive, view_name: &str) {
    siv.call_on_name("article_layout", |view: &mut LinearLayout| {
        if let Some(i) = view.find_child_from_name(view_name) {
            log::debug!("Removing the {} from the article_layout", view_name);
            view.remove_child(i);
        } else {
            log::warn!("Couldn't find the {}", view_name);
        }
    });
}
