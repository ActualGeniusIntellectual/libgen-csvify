extern crate csv;
extern crate env_logger;
extern crate log;
extern crate rayon;
extern crate sqlparser;

use chrono::Local;
use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;
use log::{debug, error, info};
use rayon::prelude::*;
use sqlparser::ast::Query;
use sqlparser::ast::SetExpr::Values;
use sqlparser::ast::Statement;
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::Parser;
use std::fs::File;
use std::io::{prelude::*, BufReader};

// Predicate if line begins with INSERT into given table
fn predicate(line: &String, table: &str) -> bool {
    line.starts_with(format!("INSERT INTO `{}`", table).as_str())
}

fn read_lines(filename: &str, table: &str) -> Vec<String> {
    info!("Reading lines from {}", filename);

    let mut result = Vec::new();

    let file = File::open(filename).unwrap();
    let reader = BufReader::new(file);

    for line in reader.lines() {
        // Push if line begins with INSERT
        let line = line.unwrap();
        debug!("Line: {}", line);
        if predicate(&line, table) {
            debug!("Pushing line: {}", line);
            result.push(line.to_string());
        }
    }

    result
}

fn parse_sql(sql: &String) -> Statement {
    let dialect = MySqlDialect {};
    let sql = sql.as_str();
    debug!("Parsing SQL: {}", sql);
    // Parse SQL
    let ast = Parser::parse_sql(&dialect, sql);

    match ast {
        Ok(ast) => {
            // Get first statement
            let insert = ast.first().unwrap().clone();

            insert
        }
        Err(e) => {
            error!("Error parsing SQL: {}", e);
            // Print sql
            panic!("{}", sql);
        }
    }
}

// Get column names from SQL
fn column_names(sql: &String) -> Vec<String> {
    let insert = parse_sql(sql);

    match insert {
        Statement::Insert { columns, .. } => columns
            .iter()
            .map(|c| c.to_string())
            .map(|s| s.replace("`", ""))
            .collect(),
        _ => Vec::new(),
    }
}

// Get query object from statement
fn query(insert: Statement) -> Option<Query> {
    let src = match insert {
        Statement::Insert { source, .. } => source,
        _ => None,
    };

    src.map(|s| *s)
}

// Get values object from query
fn values(query: Query) -> sqlparser::ast::Values {
    let values = match query {
        Query { body, .. } => body,
    };

    let values = values.clone();
    let values = *values;

    // Iterate over values
    let val = match values {
        Values(values) => Some(values),
        _ => None,
    };

    let val = val.unwrap();

    val
}

// Get rows from values
fn rows(val: sqlparser::ast::Values) -> Vec<Vec<String>> {
    let mut rows = Vec::<Vec<String>>::new();
    val.rows.iter().for_each(|row| {
        // iterate over columns
        let mut single_row = Vec::new();
        row.iter().for_each(|col| {
            // Match for number and SingleQuotedString
            let string = match col {
                sqlparser::ast::Expr::Value(sqlparser::ast::Value::Number(num, _)) => num,
                sqlparser::ast::Expr::Value(sqlparser::ast::Value::SingleQuotedString(s)) => s,
                _ => {
                    panic!("Unknown type");
                }
            };

            single_row.push(string.to_string());
        });

        rows.push(single_row);
    });

    debug!("Rows: {}", rows.len());
    rows
}

// Use rayon to parallelize map
fn rayonize(contents: Vec<&String>) -> Vec<Vec<String>> {
    contents
        .into_par_iter()
        .filter_map(|line| {
            let insert = parse_sql(line);

            // Get query values
            let src = query(insert);

            // Get values
            src.map(|src| {
                let val = values(src);

                // iterate over rows
                let rows = rows(val);

                rows
            })
        })
        .flatten()
        .collect()
}

pub fn write_csv(output_file: String, headers: Vec<String>, rows: Vec<Vec<String>>) {
    // Write to csv file
    info!("Writing to {}", output_file);
    let mut writer = csv::Writer::from_path(output_file).unwrap();
    writer.write_record(&headers).unwrap();
    for row in rows {
        writer.write_record(&row).unwrap();
    }

    writer.flush().unwrap();
}

pub fn logic(input_file: &str, table: &str) {
    // Log using info both input_file and table
    info!("Input file: {}", input_file);
    info!("Table: {}", table);
    // Create output filename from input filename by replacing .sql with .csv
    let output_file = input_file.replace(".sql", ".csv");

    // Get file contents, by lines
    let contents = read_lines(input_file, table);
    let mut contents = contents.iter();

    // Get column names
    let headers = column_names(contents.next().unwrap());
    info!("Headers: {:?}", headers);

    let contents = contents.collect::<Vec<&String>>();

    // Using rayon to parallelize map
    let rows = rayonize(contents);
    info!("Rows: {}", rows.len());

    // Write to csv file
    write_csv(output_file, headers, rows);

    // Log using info both input_file and table
    info!("Finished {} {}", input_file, table);
}

pub mod my_reader {
    use std::{
        fs::File,
        io::{self, prelude::*},
    };

    pub struct BufReader {
        reader: io::BufReader<File>,
    }

    impl BufReader {
        pub fn open(path: impl AsRef<std::path::Path>) -> io::Result<Self> {
            let file = File::open(path)?;
            let reader = io::BufReader::new(file);

            Ok(Self { reader })
        }

        pub fn read_line<'buf>(
            &mut self,
            buffer: &'buf mut String,
        ) -> Option<io::Result<&'buf mut String>> {
            buffer.clear();

            self.reader
                .read_line(buffer)
                .map(|u| if u == 0 { None } else { Some(buffer) })
                .transpose()
        }
    }
}


// Files to parse
const LIBGEN_COMPACT: &str = "libgen_compact.sql";

// Tables to parse
const UPDATED: &str = "updated";

fn main() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(rayon::current_num_threads())
        .build_global()
        .unwrap();

    // Initialize logger
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .init();
    log::info!("Starting");

    logic(LIBGEN_COMPACT, UPDATED);
}
