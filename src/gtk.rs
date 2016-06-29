extern crate clap;
extern crate gtk;

extern crate amortization;

use std::path::{PathBuf};

use clap::{App};
use gtk::prelude::*;
use gtk::{Button, FileChooserDialog, FileChooserAction, MenuBar, MenuItem, Window, WindowType};

// Opens a file picker and returns the selected file.
fn get_db_file(parent: &Window) -> Option<PathBuf> {
    const OK: i32 = 1;
    const CANCEL: i32 = 0;

    let dialog: FileChooserDialog = FileChooserDialog::new(Some("Open Database"), Some(parent), FileChooserAction::Open);
    // TODO: figure out how to use ButtonsType enum
    dialog.add_button("_OK", OK);
    dialog.add_button("_Cancel", CANCEL);

    let res = dialog.run();
    println!("Response: {}", res);

    let filename = dialog.get_filename();
    dialog.destroy();

    if res == OK {
        filename
    } else {
        None
    }
}

fn new_db_file(parent: &Window) -> Option<PathBuf> {
    const OK: i32 = 1;
    const CANCEL: i32 = 0;

    let dialog: FileChooserDialog = FileChooserDialog::new(Some("Create Database"), Some(parent), FileChooserAction::Save);
    // TODO: figure out how to use ButtonsType enum
    dialog.add_button("_OK", OK);
    dialog.add_button("_Cancel", CANCEL);

    let res = dialog.run();
    println!("Response: {}", res);

    let filename = dialog.get_filename();
    dialog.destroy();

    if res == OK {
        if let Some(db_path) = filename {
            let p = db_path.clone();
            amortization::init_db(p.as_path());
            Some(db_path)
        } else {
            filename
        }
    } else {
        None
    }
}

fn main() {
    App::new("Amortization Calculator")
                          .version("0.1.0")
                          .author("T. Jameson Little <t.jameson.little@gmail.com>")
                          .about("Calculates an amortization table")
                          .get_matches();

    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }

    let window = Window::new(WindowType::Toplevel);
    window.set_title("Amortization Calculator");
    window.set_default_size(350, 70);

    let v_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // menu

    let menubar = MenuBar::new();

    let file = MenuItem::new_with_label("File");

    let file_menu = gtk::Menu::new();

    let new = MenuItem::new_with_label("New");
    let open = MenuItem::new_with_label("Open");
    let quit = MenuItem::new_with_label("Quit");

    {
        let w = window.clone();
        new.connect_activate(move|_| {
            println!("New thing");
            let db_file = new_db_file(&w);
            match db_file {
                Some(file) => println!("File path: {}", file.display()),
                None => println!("Nada"),
            };
        });
    }
    {
        let w = window.clone();
        open.connect_activate(move |_| {
            let db_file = get_db_file(&w);
            match db_file {
                Some(file) => println!("File path: {}", file.display()),
                None => println!("Nada"),
            };
        });
    }
    quit.connect_activate(|_| {
        gtk::main_quit();
    });

    file_menu.add(&new);
    file_menu.add(&open);
    file_menu.add(&quit);
    file.set_submenu(Some(&file_menu));
    menubar.append(&file);

    // window contents

    let button = Button::new_with_label("Click me!");

    v_box.pack_start(&menubar, false, false, 0);
    v_box.pack_start(&button, true, true, 0);
    window.add(&v_box);

    window.show_all();

    window.connect_delete_event(|_, _| {
        println!("We're going down!");
        gtk::main_quit();
        Inhibit(false)
    });

    button.connect_clicked(|_| {
        println!("Clicked!");
    });

    gtk::main();
}
