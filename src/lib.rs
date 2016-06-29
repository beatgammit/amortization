#[macro_use]
extern crate log;
extern crate rusqlite;
extern crate time;

use std::path::Path;
use rusqlite::Connection;
use time::Timespec;

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
pub struct Loan {
    pub id: i32,
    pub name: String,
    pub payment: f64,
    pub balance: f64,
    pub periods: i32,
    pub apr: f64,
    pub start_time: Timespec,
    pub time_created: Timespec,
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

    pub fn new(name: String, principal: f64, periods: i32, apr: f64, start_time: Timespec) -> Loan {
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

pub fn init_db(path: &Path) {
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

pub fn create_loan(db: &Path, loan: Loan) {
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

pub fn commit_transaction(db: &Path, name: String, amount: f64, extra: bool, date: Timespec) -> rusqlite::Result<()> {
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


