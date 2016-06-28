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
struct Transaction {
    id: i32,
    name: String,
    principal: f64,
    interest: f64,
    date: Timespec,
    time_created: Timespec,
}

#[derive(Debug)]
struct Loan {
    id: i32,
    name: String,
    payment: f64,
    balance: f64,
    periods: i32,
    apr: f64,
    start_time: Timespec,
    time_created: Timespec,
}

impl Loan {
    fn load_from_db(conn: &Connection, name: &String) -> rusqlite::Result<Loan> {
        let name = name.clone();
        conn.query_row("SELECT id, payment, balance, periods, apr, start_time, time_created FROM loans WHERE name = $0", &[&name], |row| {
            Loan{
                id: row.get(0),
                name: name.to_string(),
                payment: row.get(1),
                balance: row.get(2),
                periods: row.get(3),
                apr: row.get(4),
                start_time: row.get(5),
                time_created: row.get(6),
            }
        })
    }

    fn new(name: String, principal: f64, periods: i32, apr: f64, start_time: Timespec) -> Loan {
        Loan{
            id: 0,
            name: name.clone(),
            payment: Loan::calc_payment(principal, periods, apr),
            balance: principal,
            periods: periods,
            apr: apr,
            start_time: start_time,
            time_created: time::get_time(),
        }
    }

    fn calc_payment(principal: f64, periods: i32, apr: f64) -> f64 {
        let monthly_apr = apr / 100.0 / 12.0;

        (monthly_apr / (1.0 - ((1.0 + monthly_apr).powf(-periods as f64))))*principal
    }
}

impl Loan {
    fn calc_interest_payment(&self) -> f64 {
        let monthly_apr = self.apr / 12f64 / 100f64;
        self.balance * monthly_apr
    }
}

fn init_db(path: &Path) {
    let conn = Connection::open(path).unwrap();
    let res = conn.execute_batch("
            BEGIN;
            CREATE TABLE IF NOT EXISTS loans (
                  id              INTEGER PRIMARY KEY,
                  name            TEXT NOT NULL,
                  payment         REAL NOT NULL,
                  balance         REAL NOT NULL,
                  periods         INTEGER NOT NULL,
                  apr             REAL NOT NULL,
                  start_time      TEXT NOT NULL,
                  time_created    TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS transactions (
                  id              INTEGER PRIMARY KEY,
                  name            TEXT NOT NULL,
                  principal       REAL NOT NULL,
                  interest        REAL NOT NULL,
                  from_account    TEXT,
                  to_account      TEXT,
                  date            TEXT NOT NULL,
                  time_created    TEXT NOT NULL
            );
            COMMIT;
        ");

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
    let res = conn.execute("INSERT INTO loans (name, payment, balance, periods, apr, start_time, time_created)
                  VALUES ($1, $2, $3, $4, $5, $6, $7)",
                 &[&loan.name, &loan.payment, &loan.balance, &loan.periods, &loan.apr, &loan.start_time, &loan.time_created]);

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

    Loan::new(name.to_string(), balance, term * 12, apr, start_time)
}

fn create_transaction_from_args(matches: &ArgMatches) -> (String, f64, bool, Timespec){
    let name = matches.value_of("name").unwrap();
    let amount: f64 = matches.value_of("amount").unwrap().parse().unwrap();
    let extra = matches.is_present("extra");

    let date: Timespec = if matches.is_present("date") {
        match time::strptime(matches.value_of("date").unwrap(), "%F") {
            Ok(t) => t.to_timespec(),
            Err(err) => {
                error!("Error parsing time: {}", err);
                std::process::exit(1);
            },
        }
    } else {
        time::get_time()
    };

    (name.to_string(), amount, extra, date)
}

fn commit_transaction(db: &Path, name: String, amount: f64, extra: bool, date: Timespec) -> rusqlite::Result<()> {
    let conn = try!(Connection::open(db));
    let loan = try!(Loan::load_from_db(&conn, &name));

    let transaction = {
        let (interest, principal) = if extra {
            (0f64, amount)
        } else {
            let interest = loan.calc_interest_payment();
            if loan.payment > amount {
                println!("Amount paid is insufficient payment. Expected {}, got {}", loan.payment, amount);
                std::process::exit(1);
            }
            (interest, amount - interest)
        };

        Transaction{
            id: 0,
            name: name,
            principal: principal,
            interest: interest,
            date: date,
            time_created: time::get_time(),
        }
    };

    {
        let mut conn = conn;
        let tx = try!(conn.transaction());

        try!(tx.execute("INSERT INTO transactions (name, principal, interest, date, time_created)
                    VALUES ($1, $2, $3, $4, $5)",
                   &[&transaction.name, &transaction.principal, &transaction.interest, &transaction.date, &transaction.time_created]));
        try!(tx.execute("UPDATE loans SET balance = balance - $0 WHERE name = $1", &[&transaction.principal, &transaction.name]));
        try!(tx.commit());
    }

    println!("Payment received. You paid ${:.2} towards the balance, ${:.2} in interest and have ${:.2} remaining on your loan.", transaction.principal, transaction.interest, loan.balance - transaction.principal);
    Ok(())
}

impl Amortizer {
    fn print_loan(&self, loan: Loan) {
        println!("{}: Balance = ${:.2}, APR = {:.2}%", loan.name, loan.balance, loan.apr);
        debug!("Loan details: {:?}", loan);

        let monthly_apr = loan.apr / 12f64 / 100f64;
        if self.verbosity > 0 {
            println!("Monthly payment: {:.2}", loan.payment);
        } else {
            return;
        }

        let mut date = time::at(loan.start_time);
        date.tm_mday = 1;
        let mut balance = loan.balance;
        for i in 1..loan.periods+1 {
            let interest = balance * monthly_apr;
            let mut principal = loan.payment - interest;
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
        let mut stmt = conn.prepare("SELECT id, name, payment, balance, periods, apr, start_time, time_created FROM loans WHERE name = $0").unwrap();

        let loan_iter = match stmt.query_map(&[&name], |row| {
            Loan {
                id: row.get(0),
                name: row.get(1),
                payment: row.get(2),
                balance: row.get(3),
                periods: row.get(4),
                apr: row.get(5),
                start_time: row.get(6),
                time_created: row.get(7),
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
        let mut stmt = conn.prepare("SELECT id, name, payment, balance, periods, apr, start_time, time_created FROM loans").unwrap();

        let loan_iter = match stmt.query_map(&[], |row| {
            Loan {
                id: row.get(0),
                name: row.get(1),
                payment: row.get(2),
                balance: row.get(3),
                periods: row.get(4),
                apr: row.get(5),
                start_time: row.get(6),
                time_created: row.get(7),
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
                          .subcommand(SubCommand::with_name("pay")
                                      .about("Pay a loan")
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
                                      .arg(Arg::with_name("amount")
                                          .short("a")
                                          .long("amount")
                                          .takes_value(true)
                                          .required(true)
                                          .help("payment amount"))
                                      .arg(Arg::with_name("extra")
                                          .short("e")
                                          .long("extra")
                                          .takes_value(false)
                                          .help("this payment goes 100% to principal"))
                                      .arg(Arg::with_name("date")
                                          .long("date")
                                          .short("d")
                                          .takes_value(true)
                                          .help("date of payment (if omitted, current date assumed)"))
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

    if let Some(matches) = matches.subcommand_matches("pay") {
        let db = matches.value_of("DB").unwrap();
        let (name, amount, extra, date) = create_transaction_from_args(matches);
        match commit_transaction(Path::new(db), name, amount, extra, date) {
            Err(err) => {
                println!("Error saving to database: {}", err);
            },
            _ => (),
        };
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
