extern crate clap;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate rusqlite;
extern crate time;

use std::path::Path;

use clap::{Arg, App, SubCommand, ArgMatches};
use rusqlite::Connection;
use time::Timespec;

struct Amortizer {
    verbosity: u64,
}

#[derive(Debug)]
struct Loan {
    id: i32,
    name: String,
    balance: f64,
    periods: i32,
    apr: f64,
    start_time: Timespec,
    time_created: Timespec,
}

fn init_db(path: &Path) {
    let conn = Connection::open(path).unwrap();
    let res = conn.execute("CREATE TABLE loans (
                  id              INTEGER PRIMARY KEY,
                  name            TEXT NOT NULL,
                  balance         REAL NOT NULL,
                  periods         INTEGER NOT NULL,
                  apr             REAL NOT NULL,
                  start_time      TEXT NOT NULL,
                  time_created    TEXT NOT NULL
                  )", &[]);

    match res {
        Ok(_) => info!("Database successfully created"),
        Err(err) => {
            error!("Error creating database: {}", err);
            std::process::exit(1);
        }
    };
}

fn create_loan(db: &Path, loan: Loan) {
    let conn = Connection::open(db).unwrap();
    let res = conn.execute("INSERT INTO loans (name, balance, periods, apr, start_time, time_created)
                  VALUES ($1, $2, $3, $4, $5, $6)",
                 &[&loan.name, &loan.balance, &loan.periods, &loan.apr, &loan.start_time, &loan.time_created]);

    match res {
        Ok(_) => info!("Added loan: {}", loan.name),
        Err(err) => {
            error!("Error adding loan {}: {}", loan.name, err);
            std::process::exit(1);
        }
    };
}

fn create_loan_from_args(matches: &ArgMatches) -> Loan {
    let name = matches.value_of("name").unwrap();
    let balance: f64 = matches.value_of("balance").unwrap().parse().unwrap();
    let apr: f64 = matches.value_of("apr").unwrap().parse().unwrap();
    let term: i32 = matches.value_of("term").unwrap().parse().unwrap();

    let start_time: Timespec = if matches.is_present("start") {
        match time::strptime(matches.value_of("start").unwrap(), "%F") {
            Ok(t) => t.to_timespec(),
            Err(err) => {
                error!("Error parsing time: {}", err);
                std::process::exit(1);
            },
        }
    } else {
        time::get_time()
    };

    Loan{
        id: 0,
        name: name.to_string(),
        balance: balance,
        periods: term * 12,
        apr: apr,
        start_time: start_time,
        time_created: time::get_time(),
    }
}

impl Amortizer {
    fn print_loan(&self, loan: Loan) {
        println!("{}: Balance = ${:.2}, APR = {:.2}%", loan.name, loan.balance, loan.apr);
        debug!("Loan details: {:?}", loan);

        let monthly_apr = loan.apr / 12f64 / 100f64;
        let monthly_payment = (monthly_apr /(1f64-((1f64+monthly_apr).powf(-loan.periods as f64))))*loan.balance;
        if self.verbosity > 0 {
            println!("Monthly payment: {:.2}", monthly_payment);
        } else {
            return;
        }

        let mut date = time::at(loan.start_time);
        let mut balance = loan.balance;
        for i in 1..loan.periods+1 {
            let interest = balance * monthly_apr;
            let mut principal = monthly_payment - interest;
            if principal > balance {
                principal = balance;
            }
            balance -= principal;

            date.tm_mon += 1;
            if date.tm_mon == 12 {
                date.tm_mon -= 12;
                date.tm_year += 1;
            }

            if self.verbosity > 1 {
                println!("{}: Interest = {:.2}, Principal = {:.2}, Balance: {:.2}", time::strftime("%F", &date).unwrap(), interest, principal, balance);
            }
            if balance <= 0f64 {
                println!("Congrats, you'll pay off your loan {} months early!", loan.periods - i);
                break;
            }
        }
    }

    fn query_loan(&self, db: &Path, name: String) -> Option<Loan> {
        let conn = Connection::open(db).unwrap();
        let mut stmt = conn.prepare("SELECT id, name, balance, periods, apr, start_time, time_created FROM loans WHERE name = $0").unwrap();

        let loan_iter = match stmt.query_map(&[&name], |row| {
            Loan {
                id: row.get(0),
                name: row.get(1),
                balance: row.get(2),
                periods: row.get(3),
                apr: row.get(4),
                start_time: row.get(5),
                time_created: row.get(6),
            }
        }) {
            Ok(iter) => iter,
            Err(err) => {
                error!("Error with statement: {}", err);
                std::process::exit(1);
            }
        };

        for res in loan_iter {
            return Some(res.unwrap());
        }
        return None
    }

    fn print_loans(&self, db: &Path) {
        let conn = Connection::open(db).unwrap();
        let mut stmt = conn.prepare("SELECT id, name, balance, periods, apr, start_time, time_created FROM loans").unwrap();

        let loan_iter = match stmt.query_map(&[], |row| {
            Loan {
                id: row.get(0),
                name: row.get(1),
                balance: row.get(2),
                periods: row.get(3),
                apr: row.get(4),
                start_time: row.get(5),
                time_created: row.get(6),
            }
        }) {
            Ok(iter) => iter,
            Err(err) => {
                error!("Error with statement: {}", err);
                std::process::exit(1);
            }
        };

        for res in loan_iter {
            let loan = res.unwrap();
            self.print_loan(loan);
        }
    }
}

fn main() {
    env_logger::init().unwrap();

    let matches = App::new("Amortization Calculator")
                          .version("0.1.0")
                          .author("T. Jameson Little <t.jameson.little@gmail.com>")
                          .about("Calculates an amortization table")
                          .arg(Arg::with_name("DB")
                               .help("Database to use")
                               .index(1))
                          .arg(Arg::with_name("loan")
                               .help("Loan to query")
                               .index(2))
                          .arg(Arg::with_name("v")
                               .short("v")
                               .multiple(true)
                               .help("Sets the level of verbosity"))
                          .subcommand(SubCommand::with_name("init")
                                      .about("Initializes the database")
                                      .version("0.1.0")
                                      .author("T. Jameson Little <t.jameson.little@gmail.com>")
                                      .arg(Arg::with_name("DB")
                                           .help("Sets the database name")
                                           .required(true)
                                           .index(1))
                                      )
                          .subcommand(SubCommand::with_name("create")
                                      .about("Creates a new loan")
                                      .version("0.1.0")
                                      .author("T. Jameson Little <t.jameson.little@gmail.com>")
                                      .arg(Arg::with_name("DB")
                                           .help("Database to use")
                                           .required(true)
                                           .index(1))
                                      .arg(Arg::with_name("name")
                                           .help("Name of loan")
                                           .required(true)
                                           .index(2))
                                      .arg(Arg::with_name("balance")
                                          .short("b")
                                          .long("balance")
                                          .takes_value(true)
                                          .required(true)
                                          .help("balance of the loan"))
                                      .arg(Arg::with_name("start")
                                          .long("start")
                                          .takes_value(true)
                                          .help("first payment due date"))
                                      .arg(Arg::with_name("apr")
                                          .short("a")
                                          .long("apr")
                                          .takes_value(true)
                                          .required(true)
                                          .help("apr"))
                                      .arg(Arg::with_name("term")
                                          .short("t")
                                          .long("term")
                                          .takes_value(true)
                                          .required(true)
                                          .help("apr"))
                                      )
                          .get_matches();

    let app = Amortizer{
        verbosity: matches.occurrences_of("v"),
    };

    if let Some(matches) = matches.subcommand_matches("init") {
        let db = matches.value_of("DB").unwrap();
        init_db(Path::new(db));
        return;
    }

    if let Some(matches) = matches.subcommand_matches("create") {
        let db = matches.value_of("DB").unwrap();
        let loan = create_loan_from_args(matches);
        create_loan(Path::new(db), loan);
        return;
    }

    if !matches.is_present("DB") {
        println!("Must provide the database to operate on.");
        std::process::exit(1);
    }
    let db = Path::new(matches.value_of("DB").unwrap());
    if matches.is_present("name") {
        let name = matches.value_of("name").unwrap();
        let loan = app.query_loan(db, name.to_string());
        if let Some(loan) = loan {
            app.print_loan(loan);
        } else {
            println!("Could not find loan with the name: {}", name);
            std::process::exit(1);
        }
    } else {
        app.print_loans(db);
    }
}
