use std::env;

mod dev;
mod export;
mod routes;
mod scaffold;

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_usage();
        std::process::exit(1);
    }

    match args.remove(0).as_str() {
        "new" => {
            if let Err(err) = scaffold::handle_new(&args) {
                eprintln!("scaffold failed: {err}");
                eprintln!("Usage: pilcrow new <dir> [--with-auth] [--with-postgres]");
                std::process::exit(1);
            }
        }
        "dev" => {
            if let Err(err) = dev::handle_dev(&args) {
                eprintln!("dev server failed: {err}");
                std::process::exit(1);
            }
        }
        "export" => {
            if let Err(err) = export::handle_export(&args) {
                eprintln!("export failed: {err}");
                eprintln!("Usage: pilcrow export [<dir>]");
                std::process::exit(1);
            }
        }
        "routes" => {
            if let Err(err) = routes::handle_routes(&args) {
                eprintln!("routes failed: {err}");
                eprintln!("Usage: pilcrow routes [<app-dir>]");
                std::process::exit(1);
            }
        }
        _ => {
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  pilcrow new <dir> [--with-auth] [--with-postgres]");
    eprintln!("  pilcrow dev");
    eprintln!("  pilcrow export [<dir>]");
    eprintln!("  pilcrow routes [<app-dir>]");
}
