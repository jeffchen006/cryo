use std::collections::HashMap;

use chrono::{DateTime, Local};
use colored::Colorize;
use thousands::Separable;

use crate::{
    chunks::chunk_ops::ValueToString, ChunkData, ChunkStats, CollectError, ColumnType, Datatype,
    Dim, ExecutionEnv, FileOutput, Partition, Query, Source, Table,
};
use std::path::PathBuf;

const TITLE_R: u8 = 0;
const TITLE_G: u8 = 225;
const TITLE_B: u8 = 0;
const ERROR_R: u8 = 225;
const ERROR_G: u8 = 0;
const ERROR_B: u8 = 0;

/// summary of a freeze
#[derive(Debug, Default)]
pub struct FreezeSummary {
    /// partitions completed
    pub completed: Vec<Partition>,
    /// partitions skipped
    pub skipped: Vec<Partition>,
    /// partitions errored
    pub errored: Vec<(Option<Partition>, CollectError)>,
}

pub(crate) fn print_header<A: AsRef<str>>(header: A) {
    let header_str = header.as_ref().white().bold();
    let underline = "─".repeat(header_str.len()).truecolor(TITLE_R, TITLE_G, TITLE_B);
    println!("{}", header_str);
    println!("{}", underline);
}

pub(crate) fn print_header_error<A: AsRef<str>>(header: A) {
    let header_str = header.as_ref().white().bold();
    let underline = "─".repeat(header_str.len()).truecolor(ERROR_R, ERROR_G, ERROR_B);
    println!("{}", header_str);
    println!("{}", underline);
}

fn print_bullet<A: AsRef<str>, B: AsRef<str>>(key: A, value: B) {
    let bullet_str = "- ".truecolor(TITLE_R, TITLE_G, TITLE_B);
    let key_str = key.as_ref().white().bold();
    let value_str = value.as_ref().truecolor(170, 170, 170);
    let colon_str = ": ".truecolor(TITLE_R, TITLE_G, TITLE_B);
    println!("{}{}{}{}", bullet_str, key_str, colon_str, value_str);
}

fn print_bullet_indent<A: AsRef<str>, B: AsRef<str>>(key: A, value: B, indent: usize) {
    let bullet_str = "- ".truecolor(TITLE_R, TITLE_G, TITLE_B);
    let key_str = key.as_ref().white().bold();
    let value_str = value.as_ref().truecolor(170, 170, 170);
    let colon_str = ": ".truecolor(TITLE_R, TITLE_G, TITLE_B);
    println!("{}{}{}{}{}", " ".repeat(indent), bullet_str, key_str, colon_str, value_str);
}

pub(crate) fn print_cryo_intro(
    query: &Query,
    source: &Source,
    sink: &FileOutput,
    env: &ExecutionEnv,
    n_chunks_remaining: u64,
) {
    print_header("cryo parameters");
    let datatype_strs: Vec<_> = query.schemas.keys().map(|d| d.name()).collect();
    print_bullet("data", "");
    print_bullet_indent("datatypes", datatype_strs.join(", "), 4);
    // let rpc_url = cli::parse_rpc_url(args);
    // print_bullet("provider", rpc_url);
    print_chunks(&query.partitions);

    print_bullet("source", "");
    print_bullet_indent("network", &sink.prefix, 4);
    print_bullet_indent("rpc url", &source.rpc_url, 4);
    match source.max_requests_per_second {
        Some(max_requests_per_second) => print_bullet_indent(
            "max requests per second",
            max_requests_per_second.separate_with_commas(),
            4,
        ),
        None => print_bullet_indent("max requests per second", "unlimited", 4),
    };
    match source.max_concurrent_requests {
        Some(max_concurrent_requests) => print_bullet_indent(
            "max concurrent requests",
            max_concurrent_requests.separate_with_commas(),
            4,
        ),
        None => print_bullet_indent("max concurrent requests", "unlimited", 4),
    };
    match source.max_concurrent_chunks {
        Some(max_concurrent_chunks) => print_bullet_indent(
            "max concurrent chunks",
            max_concurrent_chunks.separate_with_commas(),
            4,
        ),
        None => print_bullet_indent("max concurrent chunks:", "unlimited", 4),
    };

    if query.schemas.contains_key(&Datatype::Logs) {
        print_bullet_indent("inner request size", source.inner_request_size.to_string(), 4);
    };

    print_bullet("output", "");
    if let Some(partition) = query.partitions.first() {
        let stats = partition.stats();
        if let Some(dim) = query.partitioned_by.first() {
            if dim == &Dim::BlockNumber {
                if let Some(block_numbers) = stats.block_numbers {
                    let chunk_size = block_numbers.chunk_size;
                    print_bullet_indent("chunk size", chunk_size.separate_with_commas(), 4);
                }
            }
        }
    }
    print_bullet_indent("n chunks", query.partitions.len().separate_with_commas(), 4);
    print_bullet_indent("chunks remaining", n_chunks_remaining.to_string(), 4);
    print_bullet_indent("output format", sink.format.as_str(), 4);
    print_bullet_indent("output dir", sink.output_dir.clone().to_string_lossy(), 4);

    // print report path
    let report_path = if env.report && n_chunks_remaining > 0 {
        match super::reports::get_report_path(env, sink, true) {
            Ok(report_path) => {
                let stripped_path: PathBuf = match report_path.strip_prefix(sink.output_dir.clone())
                {
                    Ok(stripped) => PathBuf::from("$OUTPUT_DIR").join(PathBuf::from(stripped)),
                    Err(_) => report_path,
                };
                Some(stripped_path)
            }
            _ => None,
        }
    } else {
        None
    };
    match report_path {
        None => print_bullet_indent("report file", "None", 4),
        Some(path) => print_bullet_indent("report file", path.to_str().unwrap_or("none"), 4),
    };
    let dt_start: DateTime<Local> = env.t_start.into();

    // print schemas
    print_schemas(&query.schemas);

    if env.dry {
        println!("\n\n[dry run, exiting]");
    }

    println!();
    println!();
    print_header("collecting data");
    println!("started at {}", dt_start.format("%Y-%m-%d %H:%M:%S%.3f"));
}

fn print_chunks(chunks: &[Partition]) {
    let stats = crate::types::partitions::meta_chunks_stats(chunks);
    for (dim, dim_stats) in [(Dim::BlockNumber, stats.block_numbers)].iter() {
        if let Some(dim_stats) = dim_stats {
            print_chunk(dim, dim_stats)
        }
    }

    for (dim, dim_stats) in vec![
        (Dim::TransactionHash, stats.transactions),
        (Dim::CallData, stats.call_datas),
        (Dim::Address, stats.addresses),
        (Dim::Contract, stats.contracts),
        (Dim::ToAddress, stats.to_addresses),
        (Dim::Slot, stats.slots),
        (Dim::Topic0, stats.topic0s),
        (Dim::Topic1, stats.topic1s),
        (Dim::Topic2, stats.topic2s),
        (Dim::Topic3, stats.topic3s),
    ]
    .iter()
    {
        if let Some(dim_stats) = dim_stats {
            print_chunk(dim, dim_stats)
        }
    }
}

fn print_chunk<T: Ord + ValueToString>(dim: &Dim, dim_stats: &ChunkStats<T>) {
    if dim_stats.total_values == 1 {
        print_bullet_indent(
            format!("{}", dim),
            dim_stats.min_value_to_string().unwrap_or("none".to_string()),
            4,
        );
    } else {
        match (dim_stats.min_value_to_string(), dim_stats.max_value_to_string()) {
            (Some(min), Some(max)) => print_bullet_indent(
                dim.plural_name(),
                format!(
                    "n={} min={} max={}",
                    dim_stats.total_values.separate_with_commas(),
                    min,
                    max
                ),
                4,
            ),
            _ => print_bullet_indent(
                dim.plural_name(),
                format!("n={}", dim_stats.total_values.separate_with_commas()),
                4,
            ),
        }
    }
}

fn print_schemas(schemas: &HashMap<Datatype, Table>) {
    schemas.iter().for_each(|(name, schema)| {
        println!();
        println!();
        print_schema(name, &schema.clone())
    })
}

fn print_schema(name: &Datatype, schema: &Table) {
    print_header("schema for ".to_string() + name.name().as_str());
    for column in schema.columns() {
        if let Some(column_type) = schema.column_type(column) {
            if column_type == ColumnType::UInt256 {
                for uint256_type in schema.u256_types.iter() {
                    print_bullet(
                        column.to_owned() + uint256_type.suffix().as_str(),
                        uint256_type.to_columntype().as_str(),
                    );
                }
            } else {
                print_bullet(column, column_type.as_str());
            }
        }
    }
    println!();
    if let Some(sort_cols) = schema.sort_columns.clone() {
        println!("sorting {} by: {}", name.name(), sort_cols.join(", "));
    } else {
        println!("sorting disabled for {}", name.name());
    }
    let other_columns =
        name.column_types().keys().copied().filter(|x| !schema.has_column(x)).collect::<Vec<_>>();
    let other_columns =
        if other_columns.is_empty() { "[none]".to_string() } else { other_columns.join(", ") };
    println!("\nother available columns: {}", other_columns);
}

pub(crate) fn print_cryo_conclusion(
    freeze_summary: &FreezeSummary,
    query: &Query,
    env: &ExecutionEnv,
) {
    let new_env = match env.t_end {
        None => Some(env.clone().set_end_time()),
        Some(_) => None,
    };
    let env: &ExecutionEnv = match &new_env {
        Some(e) => e,
        None => env,
    };
    let t_end = match env.t_end {
        Some(t_end) => t_end,
        _ => return,
    };
    let dt_data_done: DateTime<Local> = t_end.into();

    println!("   done at {}", dt_data_done.format("%Y-%m-%d %H:%M:%S%.3f").to_string().as_str());

    if freeze_summary.errored.is_empty() {
    } else {
        println!("...done (errors in {} chunks)", freeze_summary.errored.len())
    };
    println!();
    println!();

    if !freeze_summary.errored.is_empty() {
        print_header_error("error summary");
        println!("(errors in {} chunks)", freeze_summary.errored.len());
        let mut error_counts: HashMap<String, usize> = HashMap::new();
        for (_partition, error) in freeze_summary.errored.iter() {
            *error_counts.entry(error.to_string()).or_insert(0) += 1;
        }
        for (error, count) in error_counts.iter().take(10) {
            println!("- {} ({}x)", error, count);
        }
        if error_counts.len() > 10 {
            println!("...")
        }
        println!();
        println!();
    }

    let duration = match t_end.duration_since(env.t_start) {
        Ok(duration) => duration,
        Err(_e) => {
            println!("error computing system time, aborting");
            return
        }
    };
    let seconds = duration.as_secs();
    let millis = duration.subsec_millis();
    let total_time = (seconds as f64) + (duration.subsec_nanos() as f64) / 1e9;
    let duration_string = format!("{}.{:03} seconds", seconds, millis);

    print_header("collection summary");
    print_bullet("total duration", duration_string);
    let n_chunks = query.partitions.len();
    let n_chunks_str = n_chunks.separate_with_commas();
    let width = n_chunks_str.len();
    print_bullet("total chunks", n_chunks_str.clone());
    print_bullet_indent(
        "chunks errored",
        format!(
            "  {:>width$} / {} ({}%)",
            freeze_summary.errored.len().separate_with_commas(),
            &n_chunks_str,
            format_float((100 * freeze_summary.errored.len() / n_chunks) as f64),
            width = width
        ),
        4,
    );
    print_bullet_indent(
        "chunks skipped",
        format!(
            "  {:>width$} / {} ({}%)",
            freeze_summary.skipped.len().separate_with_commas(),
            n_chunks_str,
            format_float((100 * freeze_summary.skipped.len() / n_chunks) as f64),
            width = width
        ),
        4,
    );
    print_bullet_indent(
        "chunks collected",
        format!(
            "{:>width$} / {} ({}%)",
            freeze_summary.completed.len().separate_with_commas(),
            n_chunks_str,
            format_float((100 * freeze_summary.completed.len() / n_chunks) as f64),
            width = width
        ),
        4,
    );

    print_chunks_speeds(freeze_summary.completed.clone(), &query.partitioned_by, total_time);
}

macro_rules! print_dim_speed {
    ($chunks:expr, $partition_by:expr, $total_time:expr, $name:ident, $dim:expr) => {
        if $partition_by.contains(&$dim) {
            print_chunk_speed(
                $dim.plural_name(),
                $total_time,
                $chunks.iter().map(|c| c.$name.clone()).collect(),
            );
        };
    };
}

fn print_chunks_speeds(chunks: Vec<Partition>, partition_by: &[Dim], total_time: f64) {
    print_dim_speed!(chunks, partition_by, total_time, block_numbers, Dim::BlockNumber);
    print_dim_speed!(chunks, partition_by, total_time, transactions, Dim::TransactionHash);
    print_dim_speed!(chunks, partition_by, total_time, call_datas, Dim::CallData);
    print_dim_speed!(chunks, partition_by, total_time, addresses, Dim::Address);
    print_dim_speed!(chunks, partition_by, total_time, contracts, Dim::Contract);
    print_dim_speed!(chunks, partition_by, total_time, slots, Dim::Slot);
    print_dim_speed!(chunks, partition_by, total_time, topic0s, Dim::Topic0);
    print_dim_speed!(chunks, partition_by, total_time, topic1s, Dim::Topic1);
    print_dim_speed!(chunks, partition_by, total_time, topic2s, Dim::Topic2);
    print_dim_speed!(chunks, partition_by, total_time, topic3s, Dim::Topic3);
}

fn print_chunk_speed<T: ChunkData>(name: &str, total_time: f64, chunks: Vec<Option<Vec<T>>>) {
    let flat_chunks: Vec<T> = chunks.into_iter().flatten().flatten().collect();
    let n_completed = flat_chunks.size();
    print_unit_speeds(name.into(), n_completed, total_time);
}
fn print_unit_speeds(name: String, n_completed: u64, total_time: f64) {
    let per_second = (n_completed as f64) / total_time;
    let per_minute = per_second * 60.0;
    let per_hour = per_minute * 60.0;
    let per_day = per_hour * 24.0;

    let per_day_str = format_float(per_day);
    let per_hour_str = format_float(per_hour);
    let per_minute_str = format_float(per_minute);
    let per_second_str = format_float(per_second);

    let width = per_day_str.len();

    print_bullet(name.clone() + " collected", n_completed.separate_with_commas());
    print_bullet_indent(
        name.clone() + " per second",
        format!("{:>width$}", per_second_str, width = width - 3),
        4,
    );
    print_bullet_indent(
        name.clone() + " per minute",
        format!("{:>width$}", per_minute_str, width = width - 3),
        4,
    );
    print_bullet_indent(
        name.clone() + " per hour",
        format!("{:>width$}", per_hour_str, width = std::cmp::max(5, width - 1)),
        4,
    );
    print_bullet_indent(name + " per day", format!("{:>width$}", per_day_str, width = 6), 4);
}

fn format_float(number: f64) -> String {
    let decimal_places = 1;

    let int_part = number.trunc() as i64;
    let frac_multiplier = 10f64.powi(decimal_places as i32);
    let frac_part = (number.fract() * frac_multiplier).round() as usize;

    if frac_part == 0 {
        return format!("{}.0", int_part.separate_with_commas())
    }

    let frac_str =
        format!("{:0>width$}", frac_part, width = decimal_places).trim_end_matches('0').to_string();

    format!("{}.{}", int_part.separate_with_commas(), frac_str)
}
